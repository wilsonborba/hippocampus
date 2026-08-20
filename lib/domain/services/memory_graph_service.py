from __future__ import annotations

from typing import Iterable, Optional

from lib.core.settings import Settings, get_settings
from lib.dal.repositories.entity_repository import EntityRepository
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.dal.repositories.resource_repository import ResourceRepository
from lib.dal.repositories.tag_repository import TagRepository
from lib.domain.errors import ValidationError
from lib.domain.graph_models import GraphEdge, GraphEdgeType, GraphNode, GraphNodeType, MemoryGraph

_MEMORY_LABEL_MAX = 60


def _memory_label(memory) -> str:
    text = (memory.title or memory.summary or memory.content or memory.id).strip()
    return text if len(text) <= _MEMORY_LABEL_MAX else text[: _MEMORY_LABEL_MAX - 1] + "…"


class MemoryGraphService:
    """Builds the neutral node/edge structure behind `GET /memories/{id}/graph`.

    Reuses the same bounded-BFS shape as `MemoryService.neighbors()` (spec
    Part 4 §40-41, §121: depth and node count are always capped, never
    unlimited-recursive), but keeps the visited `Memory` rows as nodes
    (`neighbors()` discards everything but relationship edges) and folds in
    tags/entities/resources as additional node types, deduped across the
    whole traversal since one tag/entity/resource can be shared by many
    memories."""

    def __init__(
        self,
        memory_repo: MemoryRepository,
        tag_repo: TagRepository,
        entity_repo: EntityRepository,
        resource_repo: ResourceRepository,
        settings: Optional[Settings] = None,
    ) -> None:
        self._memories = memory_repo
        self._tags = tag_repo
        self._entities = entity_repo
        self._resources = resource_repo
        self._settings = settings or get_settings()

    def build_graph(
        self,
        root_ids: Iterable[str],
        depth: Optional[int] = None,
        relation_types: Optional[list[str]] = None,
        include_entities: bool = True,
        include_tags: bool = True,
        include_resources: bool = True,
        max_nodes: int = 300,
    ) -> MemoryGraph:
        roots = [r for r in root_ids if r]
        if not roots:
            # Mirrors ConsolidationService's refusal to ever operate on "the
            # whole corpus" without explicit scope (spec Part 5 §64 analog).
            raise ValidationError("build_graph requires at least one root memory id")

        max_depth = self._settings.graph_max_depth
        depth = min(max(depth or max_depth, 1), max_depth)
        relation_type_filter = set(relation_types) if relation_types else None

        memories_by_id = {}
        for root_id in roots:
            memory = self._memories.get(root_id)
            if memory is None:
                raise ValidationError(f"memory {root_id!r} not found")
            memories_by_id[root_id] = memory

        seen_memory_ids: set[str] = set(roots)
        frontier = list(roots)
        edges: list[GraphEdge] = []
        seen_relationship_ids: set[str] = set()
        node_cap_hit = False

        for _ in range(depth):
            next_frontier: list[str] = []
            for mid in frontier:
                for rel in self._memories.list_relationships(mid, relation_type=None):
                    if relation_type_filter and rel.relation_type not in relation_type_filter:
                        continue
                    # `list_relationships` is bidirectional (spec Part 5 §48):
                    # once both of an edge's endpoints have been visited, the
                    # same row would otherwise be collected twice.
                    if rel.id in seen_relationship_ids:
                        continue
                    seen_relationship_ids.add(rel.id)
                    edges.append(
                        GraphEdge(
                            source_id=rel.source_memory_id,
                            target_id=rel.target_memory_id,
                            edge_type=rel.relation_type,
                            confidence=rel.confidence,
                        )
                    )
                    other = (
                        rel.target_memory_id
                        if rel.source_memory_id == mid
                        else rel.source_memory_id
                    )
                    if other not in seen_memory_ids:
                        if len(seen_memory_ids) >= max_nodes:
                            node_cap_hit = True
                            continue
                        seen_memory_ids.add(other)
                        next_frontier.append(other)
                if node_cap_hit:
                    break
            frontier = next_frontier
            if not frontier or node_cap_hit:
                break

        for memory_id in seen_memory_ids - memories_by_id.keys():
            memory = self._memories.get(memory_id)
            if memory is not None:
                memories_by_id[memory_id] = memory

        nodes: list[GraphNode] = [
            GraphNode(
                id=memory.id,
                node_type=GraphNodeType.MEMORY.value,
                label=_memory_label(memory),
                subtitle=memory.memory_type,
                metadata={"status": memory.status, "importance": memory.importance},
            )
            for memory in memories_by_id.values()
        ]

        # only keep edges whose endpoints actually made it into the traversal
        edges = [e for e in edges if e.source_id in memories_by_id and e.target_id in memories_by_id]

        if include_tags or include_entities or include_resources:
            self._attach_auxiliary_nodes(
                memories_by_id.keys(), nodes, edges,
                include_tags=include_tags, include_entities=include_entities,
                include_resources=include_resources,
            )

        graph = MemoryGraph(
            nodes=nodes, edges=edges, root_ids=list(roots),
            truncated=node_cap_hit or len(nodes) > self._settings.graph_cluster_threshold,
        )
        return self._cluster_if_needed(graph)

    def _attach_auxiliary_nodes(
        self,
        memory_ids: Iterable[str],
        nodes: list[GraphNode],
        edges: list[GraphEdge],
        include_tags: bool,
        include_entities: bool,
        include_resources: bool,
    ) -> None:
        seen_tag_ids: set[str] = set()
        seen_entity_ids: set[str] = set()
        seen_resource_ids: set[str] = set()

        for memory_id in memory_ids:
            if include_tags:
                for tag in self._memories.list_tags(memory_id):
                    node_id = f"tag:{tag.id}"
                    if tag.id not in seen_tag_ids:
                        seen_tag_ids.add(tag.id)
                        nodes.append(
                            GraphNode(id=node_id, node_type=GraphNodeType.TAG.value, label=tag.canonical_name)
                        )
                    edges.append(
                        GraphEdge(source_id=memory_id, target_id=node_id, edge_type=GraphEdgeType.TAGGED_WITH.value)
                    )

            if include_entities:
                for link in self._memories.list_entity_links(memory_id):
                    node_id = f"entity:{link.entity_id}"
                    if link.entity_id not in seen_entity_ids:
                        seen_entity_ids.add(link.entity_id)
                        entity = self._entities.get_by_id(link.entity_id)
                        if entity is not None:
                            nodes.append(
                                GraphNode(
                                    id=node_id, node_type=GraphNodeType.ENTITY.value,
                                    label=entity.canonical_name, subtitle=entity.entity_type,
                                )
                            )
                    edges.append(
                        GraphEdge(
                            source_id=memory_id, target_id=node_id,
                            edge_type=GraphEdgeType.MENTIONS.value, confidence=link.confidence,
                        )
                    )

            if include_resources:
                for link in self._memories.list_resource_links(memory_id):
                    node_id = f"resource:{link.resource_id}"
                    if link.resource_id not in seen_resource_ids:
                        seen_resource_ids.add(link.resource_id)
                        resource = self._resources.get_by_id(link.resource_id)
                        if resource is not None:
                            nodes.append(
                                GraphNode(
                                    id=node_id, node_type=GraphNodeType.RESOURCE.value,
                                    label=resource.title or resource.uri or resource.id,
                                    subtitle=resource.resource_type,
                                )
                            )
                    edges.append(GraphEdge(source_id=memory_id, target_id=node_id, edge_type=link.relationship))

    def _cluster_if_needed(self, graph: MemoryGraph) -> MemoryGraph:
        """Hybrid small/large behavior: below the threshold, render everything
        in full detail; above it, keep the roots and their immediate
        neighborhood expanded and collapse the rest into per-(node_type,
        subtitle) cluster nodes carrying a member count."""
        threshold = self._settings.graph_cluster_threshold
        if len(graph.nodes) <= threshold:
            return graph

        root_ids = set(graph.root_ids)
        node_by_id = {n.id: n for n in graph.nodes}
        keep_ids = set(root_ids)
        for edge in graph.edges:
            if edge.source_id in root_ids:
                keep_ids.add(edge.target_id)
            if edge.target_id in root_ids:
                keep_ids.add(edge.source_id)

        clustered_nodes = [node_by_id[i] for i in keep_ids if i in node_by_id]
        groups: dict[tuple[str, Optional[str]], list[str]] = {}
        for node in graph.nodes:
            if node.id in keep_ids:
                continue
            groups.setdefault((node.node_type, node.subtitle), []).append(node.id)

        cluster_edges = [e for e in graph.edges if e.source_id in keep_ids and e.target_id in keep_ids]
        for (node_type, subtitle), member_ids in groups.items():
            cluster_id = f"cluster:{node_type}:{subtitle or 'misc'}"
            clustered_nodes.append(
                GraphNode(
                    id=cluster_id, node_type=GraphNodeType.CLUSTER.value,
                    label=f"{len(member_ids)} {node_type}(s)" + (f" • {subtitle}" if subtitle else ""),
                    subtitle=subtitle, cluster_of=member_ids,
                )
            )
            member_set = set(member_ids)
            for edge in graph.edges:
                if edge.source_id in member_set and edge.target_id in keep_ids:
                    cluster_edges.append(
                        GraphEdge(source_id=cluster_id, target_id=edge.target_id,
                                  edge_type=GraphEdgeType.MEMBER_OF_CLUSTER.value)
                    )
                elif edge.target_id in member_set and edge.source_id in keep_ids:
                    cluster_edges.append(
                        GraphEdge(source_id=edge.source_id, target_id=cluster_id,
                                  edge_type=GraphEdgeType.MEMBER_OF_CLUSTER.value)
                    )

        return MemoryGraph(
            nodes=clustered_nodes, edges=cluster_edges, root_ids=graph.root_ids, truncated=True
        )
