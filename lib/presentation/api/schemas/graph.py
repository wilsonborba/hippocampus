from __future__ import annotations

from typing import Any, Optional

from pydantic import BaseModel, ConfigDict, Field


class GraphNodeOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    node_type: str
    label: str
    subtitle: Optional[str] = None
    metadata: dict[str, Any] = Field(default_factory=dict)
    cluster_of: Optional[list[str]] = None


class GraphEdgeOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    source_id: str
    target_id: str
    edge_type: str
    confidence: Optional[float] = None


class MemoryGraphOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    nodes: list[GraphNodeOut]
    edges: list[GraphEdgeOut]
    root_ids: list[str]
    truncated: bool
