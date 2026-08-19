from __future__ import annotations

import pytest

from lib.dal.models import MemoryStatus
from lib.domain.errors import ValidationError
from lib.domain.models import MemoryInput, SearchFilters
from lib.domain.services.consolidation_service import ConsolidationService


@pytest.fixture()
def consolidation_service(memory_repo):
    return ConsolidationService(memory_repo=memory_repo)


def test_requires_memory_ids_or_filters(consolidation_service):
    with pytest.raises(ValidationError):
        consolidation_service.analyze()


def test_detects_exact_duplicate(memory_service, consolidation_service):
    a = memory_service.remember(MemoryInput(content="CouchDB is the selected document store.", tags=["project:hippocampus"]))
    b = memory_service.remember(MemoryInput(content="CouchDB is the selected document store.", tags=["project:hippocampus"]))

    report = consolidation_service.analyze(filters=SearchFilters(tags_any=["project:hippocampus"]))
    pairs = {frozenset((d.memory_a_id, d.memory_b_id)) for d in report.candidate_duplicates}
    assert frozenset((a.id, b.id)) in pairs


def test_does_not_flag_distinct_memories_as_duplicates(memory_service, consolidation_service):
    memory_service.remember(MemoryInput(content="Mapper V2 was proposed.", tags=["project:x"]))
    memory_service.remember(MemoryInput(content="Mapper V3 was selected.", tags=["project:x"]))

    report = consolidation_service.analyze(filters=SearchFilters(tags_any=["project:x"]))
    assert report.candidate_duplicates == []


def test_detects_contradiction_candidate_for_shared_entity(memory_service, consolidation_service):
    a = memory_service.remember(MemoryInput(content="The project uses MongoDB.", memory_type="decision"))
    b = memory_service.remember(MemoryInput(content="The project uses CouchDB.", memory_type="decision"))
    memory_service.associate_entity(a.id, "database", "database-choice")
    memory_service.associate_entity(b.id, "database", "database-choice")

    report = consolidation_service.analyze(memory_ids=[a.id, b.id])
    pairs = {(c.memory_a_id, c.memory_b_id) for c in report.candidate_contradictions}
    assert (a.id, b.id) in pairs
    # A detected contradiction must never be silently resolved by analyze.
    assert memory_service.get(a.id).status == MemoryStatus.ACTIVE.value
    assert memory_service.get(b.id).status == MemoryStatus.ACTIVE.value


def test_existing_relationship_suppresses_contradiction_candidate(memory_service, consolidation_service):
    a = memory_service.remember(MemoryInput(content="The project uses MongoDB.", memory_type="decision"))
    b = memory_service.remember(MemoryInput(content="The project uses CouchDB.", memory_type="decision"))
    memory_service.associate_entity(a.id, "database", "database-choice")
    memory_service.associate_entity(b.id, "database", "database-choice")
    memory_service.correct(a.id, b.id)  # already explained: b corrects a

    report = consolidation_service.analyze(memory_ids=[a.id, b.id])
    assert report.candidate_contradictions == []


def test_apply_safe_archives_newer_duplicate_and_preserves_older(memory_service, consolidation_service):
    older = memory_service.remember(MemoryInput(content="CouchDB is the selected document store."))
    newer = memory_service.remember(MemoryInput(content="CouchDB is the selected document store."))

    report = consolidation_service.apply_safe(memory_ids=[older.id, newer.id])

    assert len(report.applied_changes) == 1
    refreshed_older = memory_service.get(older.id)
    refreshed_newer = memory_service.get(newer.id)
    assert refreshed_older.status == MemoryStatus.ACTIVE.value
    assert refreshed_newer.status == MemoryStatus.ARCHIVED.value
    # both rows still exist with their original content (never deleted/rewritten)
    assert refreshed_older.content == "CouchDB is the selected document store."
    assert refreshed_newer.content == "CouchDB is the selected document store."

    relationships = memory_service.neighbors(newer.id)
    assert any(r.relation_type == "related_to" and r.target_memory_id == older.id for r in relationships)


def test_apply_safe_is_idempotent(memory_service, consolidation_service):
    older = memory_service.remember(MemoryInput(content="dup"))
    newer = memory_service.remember(MemoryInput(content="dup"))

    first = consolidation_service.apply_safe(memory_ids=[older.id, newer.id])
    second = consolidation_service.apply_safe(memory_ids=[older.id, newer.id])

    assert len(first.applied_changes) == 1
    assert len(second.applied_changes) == 0  # already archived, nothing left to do
