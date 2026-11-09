from __future__ import annotations

from dataclasses import dataclass

from lib.core.logs import get_logger
from lib.dal.remote.document_store import MemoryDocumentStore
from lib.dal.repositories.memory_repository import MemoryRepository

logger = get_logger(__name__)


@dataclass
class DocumentSyncReport:
    processed: int = 0
    synced: int = 0
    skipped_no_document: int = 0
    failed: int = 0


def run(memory_repo: MemoryRepository, document_store: MemoryDocumentStore, batch_limit: int = 50) -> DocumentSyncReport:
    """SyncMemoryDocumentWorker (spec Part 6 §34-40): refreshes the CouchDB
    tag/entity snapshot for memories that already have a document. PostgreSQL
    stays authoritative regardless of whether this succeeds (Part 6 §40 —
    failure here must never roll back the PostgreSQL relationship)."""
    report = DocumentSyncReport()
    candidates = memory_repo.list(limit=batch_limit)
    for memory in candidates:
        report.processed += 1
        if not memory.document_id:
            report.skipped_no_document += 1
            continue
        existing_doc = document_store.get(memory.id) or {}
        tags = [t.canonical_name for t in memory_repo.list_tags(memory.id)]
        entity_links = memory_repo.list_entity_links(memory.id)
        doc = {
            **existing_doc,
            "memory_id": memory.id,
            "schema_version": existing_doc.get("schema_version", 1),
            "content": memory.content,
            "summary": memory.summary,
            "tags": tags,
            "entities": [{"id": e.entity_id, "role": e.role} for e in entity_links],
        }
        if document_store.put(memory.id, doc):
            report.synced += 1
        else:
            report.failed += 1
            logger.warning("document sync failed for memory_id=%r", memory.id)
    return report
