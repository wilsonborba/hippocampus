from __future__ import annotations

from dataclasses import dataclass, field

from lib.dal.remote.document_store import MemoryDocumentStore
from lib.dal.repositories.memory_repository import MemoryRepository


@dataclass
class ReconciliationReport:
    missing_document: list[str] = field(default_factory=list)
    missing_embedding: list[str] = field(default_factory=list)


def run(
    memory_repo: MemoryRepository, document_store: MemoryDocumentStore, batch_limit: int = 200,
    dry_run: bool = True,
) -> ReconciliationReport:
    """ReconciliationWorker (spec Part 6 §53-57, Part 7 §44). Default behavior
    is inspect + report, never destructive cleanup (Part 6 §55). Orphan-document
    detection (a CouchDB doc with no matching Memory) is intentionally out of
    scope here: neither adapter exposes a cheap "list all documents" operation,
    and scanning either store wholesale doesn't fit the bounded-query principle
    (Part 7 §72) — it belongs to a dedicated admin pass once CouchDB view
    support is wired in."""
    report = ReconciliationReport()
    candidates = memory_repo.list(limit=batch_limit)
    embedded_ids = {row.memory_id for row in memory_repo.list_embeddings()}
    for memory in candidates:
        if memory.document_id and document_store.get(memory.id) is None:
            report.missing_document.append(memory.id)
        if memory.id not in embedded_ids:
            report.missing_embedding.append(memory.id)

    if not dry_run:
        # Safe repairs only (spec Part 6 §56): regenerating a missing
        # embedding/document is left to the dedicated workers, which already
        # know how to do it correctly — this worker only ever reports.
        pass

    return report
