from __future__ import annotations

import pytest

from lib.core.settings import Settings
from lib.domain.errors import ValidationError
from lib.domain.graph_models import GraphNodeType
from lib.domain.models import MemoryInput
from lib.domain.services.memory_graph_service import MemoryGraphService


def test_build_graph_requires_a_root(memory_graph_service):
    with pytest.raises(ValidationError):
        memory_graph_service.build_graph([])


def test_build_graph_unknown_root_raises(memory_graph_service):
    with pytest.raises(ValidationError):
        memory_graph_service.build_graph(["does-not-exist"])


def test_build_graph_includes_related_memories_within_depth(memory_service, memory_graph_service):
    a = memory_service.remember(MemoryInput(content="a"))
    b = memory_service.remember(MemoryInput(content="b"))
    c = memory_service.remember(MemoryInput(content="c"))
    memory_service.link(a.id, "related_to", b.id)
    memory_service.link(b.id, "related_to", c.id)

    graph = memory_graph_service.build_graph([a.id], depth=1)
    ids = {n.id for n in graph.nodes}
    assert a.id in ids and b.id in ids
    assert c.id not in ids  # two hops away, outside depth=1

    graph_deep = memory_graph_service.build_graph([a.id], depth=2)
    assert c.id in {n.id for n in graph_deep.nodes}


def test_build_graph_does_not_duplicate_edge_seen_from_both_ends(memory_service, memory_graph_service):
    a = memory_service.remember(MemoryInput(content="a"))
    b = memory_service.remember(MemoryInput(content="b"))
    memory_service.link(a.id, "related_to", b.id)

    # depth=2 revisits the same relationship from B's side too
    graph = memory_graph_service.build_graph([a.id], depth=2)
    matching = [e for e in graph.edges if e.source_id == a.id and e.target_id == b.id]
    assert len(matching) == 1


def test_build_graph_filters_by_relation_type(memory_service, memory_graph_service):
    a = memory_service.remember(MemoryInput(content="a"))
    b = memory_service.remember(MemoryInput(content="b"))
    c = memory_service.remember(MemoryInput(content="c"))
    memory_service.link(a.id, "related_to", b.id)
    memory_service.link(a.id, "contradicts", c.id)

    graph = memory_graph_service.build_graph([a.id], depth=1, relation_types=["related_to"])
    ids = {n.id for n in graph.nodes}
    assert b.id in ids
    assert c.id not in ids


def test_build_graph_includes_tags_entities_resources(memory_service, memory_graph_service):
    memory = memory_service.remember(
        MemoryInput(
            content="x", tags=["project:hippocampus"],
            entities=[{"entity_type": "person", "canonical_name": "Wilson"}],
            resources=[{"resource_type": "doc", "source_system": "plane", "external_id": "PLN-1"}],
        )
    )
    graph = memory_graph_service.build_graph([memory.id])
    node_types = {n.node_type for n in graph.nodes}
    assert GraphNodeType.TAG.value in node_types
    assert GraphNodeType.ENTITY.value in node_types
    assert GraphNodeType.RESOURCE.value in node_types


def test_build_graph_can_exclude_auxiliary_nodes(memory_service, memory_graph_service):
    memory = memory_service.remember(
        MemoryInput(content="x", tags=["project:hippocampus"])
    )
    graph = memory_graph_service.build_graph([memory.id], include_tags=False)
    assert GraphNodeType.TAG.value not in {n.node_type for n in graph.nodes}


def test_build_graph_never_crosses_workspace_boundary_via_relationship(memory_service, memory_graph_service):
    # Same setup as test_build_graph_includes_related_memories_within_depth,
    # except b/c live in a different workspace than the root a. A BFS
    # traversal must never surface b/c as nodes, nor the edges to them,
    # once workspace_id is enforced -- a relationship row alone must not be
    # enough to cross the boundary.
    a = memory_service.remember(MemoryInput(content="a", workspace_id="tenant-a"))
    b = memory_service.remember(MemoryInput(content="b", workspace_id="tenant-b"))
    c = memory_service.remember(MemoryInput(content="c", workspace_id="tenant-b"))
    memory_service.link(a.id, "related_to", b.id)
    memory_service.link(b.id, "related_to", c.id)

    graph = memory_graph_service.build_graph([a.id], depth=2, workspace_id="tenant-a")
    ids = {n.id for n in graph.nodes}
    assert ids == {a.id}
    assert graph.edges == []

    # Without workspace_id (internal/trusted caller), the full graph is
    # still reachable -- this proves the isolation is opt-in scoping, not
    # a broken relationship traversal.
    unscoped = memory_graph_service.build_graph([a.id], depth=2)
    assert {n.id for n in unscoped.nodes} == {a.id, b.id, c.id}


def test_build_graph_root_outside_workspace_is_not_found(memory_service, memory_graph_service):
    memory = memory_service.remember(MemoryInput(content="secret", workspace_id="tenant-a"))
    with pytest.raises(ValidationError):
        memory_graph_service.build_graph([memory.id], workspace_id="tenant-b")


def test_build_graph_clusters_above_threshold(memory_service, memory_repo, tag_repo, entity_repo, resource_repo):
    root = memory_service.remember(MemoryInput(content="root"))
    for i in range(10):
        leaf = memory_service.remember(
            MemoryInput(content=f"leaf-{i}", memory_type="observation", tags=[f"topic:t{i}"])
        )
        memory_service.link(root.id, "related_to", leaf.id)

    service = MemoryGraphService(
        memory_repo=memory_repo, tag_repo=tag_repo, entity_repo=entity_repo, resource_repo=resource_repo,
        settings=Settings(graph_cluster_threshold=3),
    )
    graph = service.build_graph([root.id], depth=1)
    assert graph.truncated is True
    assert any(n.node_type == GraphNodeType.CLUSTER.value for n in graph.nodes)
    # the root's direct neighborhood (the 10 leaves) must never itself be
    # collapsed, only the tags one hop further out
    assert sum(1 for n in graph.nodes if n.node_type == GraphNodeType.MEMORY.value) == 11
    assert any(n.id == root.id for n in graph.nodes)
