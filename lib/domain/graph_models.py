from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional


class GraphNodeType(str, Enum):
    MEMORY = "memory"
    ENTITY = "entity"
    TAG = "tag"
    RESOURCE = "resource"
    CLUSTER = "cluster"


class GraphEdgeType(str, Enum):
    """memory<->memory edges reuse `RelationType` values as-is (they already
    are the controlled vocabulary); these cover the other node kinds this
    graph adds on top of `MemoryService.neighbors()`."""

    TAGGED_WITH = "tagged_with"
    MENTIONS = "mentions"
    MEMBER_OF_CLUSTER = "member_of_cluster"


@dataclass
class GraphNode:
    """One node in a `MemoryGraph` — a `Memory`, `Tag`, `Entity`, `Resource`,
    or a synthetic `cluster` node standing in for a collapsed group (see
    `MemoryGraphService._cluster_if_needed`)."""

    id: str
    node_type: str
    label: str
    subtitle: Optional[str] = None
    metadata: dict[str, Any] = field(default_factory=dict)
    cluster_of: Optional[list[str]] = None


@dataclass
class GraphEdge:
    source_id: str
    target_id: str
    edge_type: str
    confidence: Optional[float] = None


@dataclass
class MemoryGraph:
    """Neutral node/edge structure with no notion of output format (spec:
    rendering — D2/Mermaid/SVG/PNG/JSON — is a separate concern layered on
    top, see `lib.domain.rendering`)."""

    nodes: list[GraphNode] = field(default_factory=list)
    edges: list[GraphEdge] = field(default_factory=list)
    root_ids: list[str] = field(default_factory=list)
    truncated: bool = False
