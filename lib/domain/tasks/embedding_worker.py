from __future__ import annotations

from dataclasses import dataclass

from lib.core.logs import get_logger
from lib.dal.models import MemoryStatus
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.domain.services.embedding import EmbeddingProvider, content_hash

logger = get_logger(__name__)


@dataclass
class EmbeddingWorkerReport:
    processed: int = 0
    embedded: int = 0
    skipped_no_provider: int = 0
    failed: int = 0


def _canonical_text(memory) -> str:
    """Deterministic, versionable representation (spec Part 6 §30-31) — not an
    arbitrary concatenation of every field."""
    return " ".join(filter(None, [memory.title, memory.summary, memory.content])).strip()


def run(memory_repo: MemoryRepository, embedding_provider: EmbeddingProvider, batch_limit: int = 50) -> EmbeddingWorkerReport:
    """GenerateEmbeddingWorker (spec Part 6 §28-33, §145). Idempotent: reruns
    skip memories whose embedding is already current for this model+text
    (compared by content hash), matching the embedding-versioning invariant."""
    report = EmbeddingWorkerReport()
    if embedding_provider.name == "none":
        return report

    candidates = memory_repo.list(
        statuses=[MemoryStatus.ACTIVE.value, MemoryStatus.SUPERSEDED.value], limit=batch_limit
    )
    existing_by_memory = {
        row.memory_id: row for row in memory_repo.list_embeddings(model=embedding_provider.name)
    }

    for memory in candidates:
        report.processed += 1
        text = _canonical_text(memory)
        if not text:
            continue
        text_hash = content_hash(text)
        existing = existing_by_memory.get(memory.id)
        if existing and existing.source_text_hash == text_hash:
            continue  # already up to date for this model/content
        vector = embedding_provider.embed(text)
        if vector is None:
            report.failed += 1
            logger.warning("embedding generation failed for memory_id=%r", memory.id)
            continue
        memory_repo.upsert_embedding(
            memory_id=memory.id, model=embedding_provider.name, model_version=None,
            dimensions=len(vector), embedding=vector, source_text_hash=text_hash,
        )
        report.embedded += 1
    return report
