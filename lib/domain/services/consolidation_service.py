from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Optional

from lib.core.settings import Settings, get_settings
from lib.core.time_utils import ensure_utc
from lib.dal.models import Memory, MemoryEventType, MemoryStatus
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.domain.errors import ValidationError
from lib.domain.models import SearchFilters
from lib.domain.services.memory_service import canonicalize_tag_filter, normalize_workspace_id

# Memory types worth checking for contradiction: durable claims that can
# meaningfully conflict (spec Part 4 §88-91). Working/episodic/observation
# entries describe events, not competing current-state claims.
_CONTRADICTION_ELIGIBLE_TYPES = {"decision", "preference", "instruction"}

# Relations that already explain a difference between two memories — if any
# of these already connects a pair, they are not an unexplained contradiction
# candidate (spec Part 4 §90-91).
_ALREADY_EXPLAINED_RELATIONS = {"supersedes", "corrects", "contradicts", "related_to", "derived_from"}


def _normalize_text(text: Optional[str]) -> str:
    return re.sub(r"\s+", " ", (text or "").strip().lower())


@dataclass(frozen=True)
class DuplicateCandidate:
    memory_a_id: str
    memory_b_id: str
    reason: str


@dataclass(frozen=True)
class ContradictionCandidate:
    memory_a_id: str
    memory_b_id: str
    reason: str


@dataclass(frozen=True)
class AppliedChange:
    action: str
    memory_id: str
    detail: str


@dataclass
class ConsolidationReport:
    candidate_duplicates: list[DuplicateCandidate] = field(default_factory=list)
    candidate_contradictions: list[ContradictionCandidate] = field(default_factory=list)
    applied_changes: list[AppliedChange] = field(default_factory=list)


class ConsolidationService:
    """Deterministic, conservative consolidation (spec Part 4 §72-92, Part 6
    §46-50). No LLM/embedding call is required — exact-duplicate and
    contradiction-candidate detection both work from structured signals
    (normalized content, shared entities, memory type), matching "deterministic
    behavior before AI inference" (Part 4 §3). Only `apply_safe`'s low-risk
    action (archiving a confirmed exact duplicate) ever mutates state; nothing
    here auto-resolves a contradiction or deletes anything (Part 6 §48)."""

    def __init__(self, memory_repo: MemoryRepository, settings: Optional[Settings] = None) -> None:
        self._memories = memory_repo
        self._settings = settings or get_settings()

    def _resolve_candidates(
        self, memory_ids: Optional[list[str]], filters: Optional[SearchFilters]
    ) -> list[Memory]:
        if not memory_ids and not filters:
            # Spec Part 5 §64: the API must never accept an unbounded
            # "consolidate everything" request.
            raise ValidationError("consolidation requires memory_ids or filters (never the whole corpus)")

        if memory_ids:
            memories = []
            for memory_id in memory_ids[: self._settings.consolidation_max_candidates]:
                memory = self._memories.get(memory_id)
                if memory is not None:
                    memories.append(memory)
            return memories

        filters = filters or SearchFilters()
        filters.limit = min(filters.limit or self._settings.consolidation_max_candidates,
                             self._settings.consolidation_max_candidates)
        if not filters.statuses:
            filters.statuses = [MemoryStatus.ACTIVE.value]
        return self._memories.list(
            workspace_id=normalize_workspace_id(filters.workspace_id) if filters.workspace_id else None,
            memory_type=filters.memory_types[0] if filters.memory_types else None,
            statuses=filters.statuses,
            tags_any=[canonicalize_tag_filter(tag) for tag in filters.tags_any] or None,
            tags_all=[canonicalize_tag_filter(tag) for tag in filters.tags_all] or None,
            created_after=filters.created_after,
            created_before=filters.created_before,
            limit=filters.limit,
        )

    def _find_duplicates(self, candidates: list[Memory]) -> list[DuplicateCandidate]:
        duplicates = []
        seen: dict[tuple[str, str], Memory] = {}
        for memory in candidates:
            key = (memory.memory_type, _normalize_text(memory.content or memory.summary or memory.title))
            if not key[1]:
                continue
            existing = seen.get(key)
            if existing is not None:
                duplicates.append(
                    DuplicateCandidate(
                        memory_a_id=existing.id, memory_b_id=memory.id,
                        reason="identical normalized content and memory_type",
                    )
                )
            else:
                seen[key] = memory
        return duplicates

    def _find_contradictions(self, candidates: list[Memory]) -> list[ContradictionCandidate]:
        eligible = [m for m in candidates if m.memory_type in _CONTRADICTION_ELIGIBLE_TYPES]
        if len(eligible) < 2:
            return []

        entities_by_memory = {
            m.id: {link.entity_id for link in self._memories.list_entity_links(m.id)} for m in eligible
        }
        contradictions = []
        for i, memory_a in enumerate(eligible):
            for memory_b in eligible[i + 1:]:
                shared_entities = entities_by_memory[memory_a.id] & entities_by_memory[memory_b.id]
                if not shared_entities:
                    continue
                text_a = _normalize_text(memory_a.content or memory_a.summary)
                text_b = _normalize_text(memory_b.content or memory_b.summary)
                if not text_a or not text_b or text_a == text_b:
                    continue
                if self._already_explained(memory_a.id, memory_b.id):
                    continue
                contradictions.append(
                    ContradictionCandidate(
                        memory_a_id=memory_a.id, memory_b_id=memory_b.id,
                        reason=f"shared entity, same {memory_a.memory_type} type, differing content "
                               "(uncertain conflict — not auto-resolved)",
                    )
                )
        return contradictions

    def _already_explained(self, memory_a_id: str, memory_b_id: str) -> bool:
        for rel in self._memories.list_relationships(memory_a_id):
            other = rel.target_memory_id if rel.source_memory_id == memory_a_id else rel.source_memory_id
            if other == memory_b_id and rel.relation_type in _ALREADY_EXPLAINED_RELATIONS:
                return True
        return False

    def analyze(
        self, memory_ids: Optional[list[str]] = None, filters: Optional[SearchFilters] = None
    ) -> ConsolidationReport:
        candidates = self._resolve_candidates(memory_ids, filters)
        return ConsolidationReport(
            candidate_duplicates=self._find_duplicates(candidates),
            candidate_contradictions=self._find_contradictions(candidates),
        )

    def apply_safe(
        self, memory_ids: Optional[list[str]] = None, filters: Optional[SearchFilters] = None
    ) -> ConsolidationReport:
        """Applies only the low-risk action from spec Part 6 §79: for a
        confirmed exact duplicate pair, archive the newer row (never delete)
        and link it back to the canonical (older) one, preserving both and
        their full history. Contradictions are always left for a human/agent
        to resolve explicitly (Part 6 §48)."""
        report = self.analyze(memory_ids, filters)
        candidates_by_id = {}
        for memory_id in {d.memory_a_id for d in report.candidate_duplicates} | {
            d.memory_b_id for d in report.candidate_duplicates
        }:
            memory = self._memories.get(memory_id)
            if memory:
                candidates_by_id[memory_id] = memory

        for dup in report.candidate_duplicates:
            memory_a = candidates_by_id.get(dup.memory_a_id)
            memory_b = candidates_by_id.get(dup.memory_b_id)
            if memory_a is None or memory_b is None:
                continue
            older, newer = (
                (memory_a, memory_b)
                if ensure_utc(memory_a.created_at) <= ensure_utc(memory_b.created_at)
                else (memory_b, memory_a)
            )
            if newer.status != MemoryStatus.ACTIVE.value:
                continue  # already consolidated in a previous run — idempotent no-op
            self._memories.create_relationship(newer.id, older.id, "related_to", source="consolidation")
            self._memories.set_status(newer.id, MemoryStatus.ARCHIVED.value)
            self._memories.add_event(
                newer.id, MemoryEventType.CONSOLIDATED.value,
                actor_type="hippocampus_consolidator", payload={"duplicate_of": older.id},
            )
            report.applied_changes.append(
                AppliedChange(action="archived_duplicate", memory_id=newer.id, detail=f"duplicate_of={older.id}")
            )

        return report
