from __future__ import annotations

import math
from datetime import datetime, timezone
from typing import Optional

from lib.core.settings import Settings, get_settings
from lib.core.time_utils import ensure_utc
from lib.dal.models import MemoryStatus
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.dal.repositories.tag_repository import TagRepository, normalize_tag_part
from lib.domain.models import RecallRequest, RecallResult
from lib.domain.services.embedding import EmbeddingProvider, cosine_similarity
from lib.domain.services.memory_service import canonicalize_tag_filter

_HALF_LIFE_DAYS = 30.0  # recency decays to ~0.5 after this many days; a soft
# signal only (spec Part 4 §47 — recency must not overpower explicit facts).


def _recency_score(reference: datetime, now: datetime) -> float:
    age_days = max((now - reference).total_seconds() / 86400.0, 0.0)
    return math.exp(-age_days / _HALF_LIFE_DAYS)


class RecallService:
    """Multi-signal recall (spec Part 4 §19-33, §144-145). Combines
    deterministic candidate sources (tags/entities/status/text) with optional
    semantic similarity, then applies a simple inspectable weighted score —
    recall must work even with zero embeddings (spec Part 4 §55, Invariant 5)."""

    def __init__(
        self,
        memory_repo: MemoryRepository,
        tag_repo: TagRepository,
        embedding_provider: EmbeddingProvider,
        settings: Optional[Settings] = None,
    ) -> None:
        self._memories = memory_repo
        self._tags = tag_repo
        self._embeddings = embedding_provider
        self._settings = settings or get_settings()

    def recall(self, request: RecallRequest) -> list[RecallResult]:
        statuses = None if request.historical else [MemoryStatus.ACTIVE.value, MemoryStatus.SUPERSEDED.value]
        limit = min(request.limit or self._settings.recall_default_limit, self._settings.recall_max_limit)
        # Cast a slightly wider deterministic net than the final limit so
        # ranking has something to actually rank (spec Part 4 §23-24 candidate
        # union), then trim after scoring.
        candidate_pool = max(limit * 4, 20)
        tag_filters = [canonicalize_tag_filter(tag) for tag in request.tags]

        candidates = self._memories.list(
            memory_type=request.memory_types[0] if request.memory_types else None,
            statuses=statuses,
            tags_any=tag_filters or None,
            entity_id=request.entity_ids[0] if request.entity_ids else None,
            resource_id=request.resource_ids[0] if request.resource_ids else None,
            text=request.query or None,
            limit=candidate_pool,
        )
        if not candidates and request.query:
            # Text filter may have excluded everything relevant to tag/entity
            # criteria alone; fall back to those without the lexical filter
            # rather than returning nothing (still deterministic, still bounded).
            candidates = self._memories.list(
                memory_type=request.memory_types[0] if request.memory_types else None,
                statuses=statuses,
                tags_any=tag_filters or None,
                entity_id=request.entity_ids[0] if request.entity_ids else None,
                resource_id=request.resource_ids[0] if request.resource_ids else None,
                limit=candidate_pool,
            )
        if not candidates:
            return []

        candidate_ids = [c.id for c in candidates]
        tags_by_memory = self._memories.tags_for_memories(candidate_ids)
        query_tag_names = {normalize_tag_part(t.split(":")[-1]) for t in request.tags}

        query_embedding = self._embeddings.embed(request.query) if request.query else None
        embeddings_by_memory: dict[str, list[float]] = {}
        if query_embedding:
            for row in self._memories.list_embeddings():
                if row.memory_id in candidate_ids:
                    embeddings_by_memory[row.memory_id] = row.embedding

        now = datetime.now(timezone.utc)
        w = self._settings
        results: list[RecallResult] = []
        for memory in candidates:
            reasons: list[str] = []
            signals: dict[str, float] = {}

            lexical = 0.0
            if request.query:
                haystack = " ".join(filter(None, [memory.title, memory.summary, memory.content])).lower()
                if request.query.lower() in haystack:
                    lexical = 1.0
                    reasons.append("lexical match")
            signals["lexical"] = lexical

            semantic = 0.0
            if query_embedding and memory.id in embeddings_by_memory:
                semantic = cosine_similarity(query_embedding, embeddings_by_memory[memory.id])
                if semantic > 0.5:
                    reasons.append("semantic similarity")
            signals["semantic"] = semantic

            memory_tags = tags_by_memory.get(memory.id, [])
            tag_hit = 0.0
            if query_tag_names and memory_tags:
                memory_tag_values = {t.value for t in memory_tags}
                overlap = query_tag_names & memory_tag_values
                if overlap:
                    tag_hit = len(overlap) / len(query_tag_names)
                    reasons.append(f"tag match: {', '.join(sorted(overlap))}")
            signals["tag"] = tag_hit

            # Entity filtering already happened at the repository level above
            # (entity_id= narrows candidates), so any candidate reaching here
            # when entity_ids was requested already matched it.
            entity_hit = 1.0 if request.entity_ids else 0.0
            if entity_hit:
                reasons.append("entity match")
            signals["entity"] = entity_hit

            importance = memory.importance or 0.0
            signals["importance"] = importance

            reference_time = ensure_utc(memory.observed_at or memory.created_at)
            recency = _recency_score(reference_time, now)
            signals["recency"] = recency

            score = (
                w.recall_weight_semantic * semantic
                + w.recall_weight_lexical * lexical
                + w.recall_weight_tag * tag_hit
                + w.recall_weight_entity * entity_hit
                + w.recall_weight_importance * importance
                + w.recall_weight_recency * recency
            )
            if memory.status == MemoryStatus.SUPERSEDED.value and not request.historical:
                # Superseded memories may still surface as supporting context,
                # but must rank below their active replacement (spec Part 4
                # §27, acceptance criteria Part 7 §128).
                score *= w.recall_superseded_penalty
                reasons.append("superseded (ranked lower)")

            results.append(
                RecallResult(memory=memory, score=round(score, 4), signals=signals, match_reasons=reasons)
            )

        results.sort(key=lambda r: r.score, reverse=True)
        return results[:limit]
