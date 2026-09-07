from __future__ import annotations

from lib.domain.models import MemoryInput, RecallRequest


def test_active_decision_outranks_superseded_one(memory_service, recall_service):
    old = memory_service.remember(MemoryInput(content="Mapper V2 is the selected architecture.", memory_type="decision"))
    new = memory_service.remember(MemoryInput(content="Mapper V3 is the selected architecture.", memory_type="decision"))
    memory_service.supersede(old.id, new.id)

    results = recall_service.recall(RecallRequest(query="mapper"))
    ids_in_order = [r.memory.id for r in results]
    assert new.id in ids_in_order
    assert old.id in ids_in_order
    assert ids_in_order.index(new.id) < ids_in_order.index(old.id)


def test_historical_query_still_returns_superseded_memory(memory_service, recall_service):
    old = memory_service.remember(MemoryInput(content="Mapper V2 is the selected architecture."))
    new = memory_service.remember(MemoryInput(content="Mapper V3 is the selected architecture."))
    memory_service.supersede(old.id, new.id)

    results = recall_service.recall(RecallRequest(query="mapper", historical=True))
    ids = {r.memory.id for r in results}
    assert old.id in ids
    assert new.id in ids


def test_tag_scoped_recall_prefers_matching_project(memory_service, recall_service):
    hippocampus_memory = memory_service.remember(
        MemoryInput(content="We chose CouchDB.", tags=["project:hippocampus"])
    )
    other_project_memory = memory_service.remember(
        MemoryInput(content="We chose CouchDB.", tags=["project:autodroid"])
    )

    results = recall_service.recall(RecallRequest(query="CouchDB", tags=["project:hippocampus"]))
    ids_in_order = [r.memory.id for r in results]
    assert ids_in_order[0] == hippocampus_memory.id
    assert other_project_memory.id not in ids_in_order or ids_in_order.index(
        hippocampus_memory.id
    ) < ids_in_order.index(other_project_memory.id)


def test_bare_tag_recall_matches_general_namespace(memory_service, recall_service):
    memory = memory_service.remember(MemoryInput(content="Data Lake access state.", tags=["KAN-805"]))

    results = recall_service.recall(RecallRequest(query="access", tags=["KAN-805"]))

    assert [r.memory.id for r in results] == [memory.id]


def test_recall_filters_by_workspace(memory_service, recall_service):
    work_memory = memory_service.remember(
        MemoryInput(workspace_id="work", content="Data Lake access state.", tags=["KAN-805"])
    )
    memory_service.remember(
        MemoryInput(workspace_id="personal", content="Data Lake access state.", tags=["KAN-805"])
    )

    results = recall_service.recall(
        RecallRequest(workspace_id="work", query="access", tags=["KAN-805"])
    )

    assert [r.memory.id for r in results] == [work_memory.id]


def test_recall_returns_nothing_for_empty_corpus(recall_service):
    assert recall_service.recall(RecallRequest(query="anything")) == []
