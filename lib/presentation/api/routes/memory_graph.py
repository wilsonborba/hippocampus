from __future__ import annotations

from typing import Optional

from fastapi import APIRouter, Depends, Query
from fastapi.responses import PlainTextResponse, Response

from lib.domain.errors import UnsupportedRenderFormatError
from lib.domain.rendering import compute_layout, render_d2, render_mermaid, render_png, render_svg
from lib.domain.services.memory_graph_service import MemoryGraphService
from lib.domain.services.memory_service import MemoryService
from lib.presentation.api.deps import get_memory_graph_service, get_memory_service
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.graph import MemoryGraphOut

router = APIRouter(prefix="/api/v1/memories", tags=["memories"])

_TEXT_MEDIA_TYPE = {"d2": "text/vnd.d2", "mermaid": "text/vnd.mermaid"}


@router.get("/{memory_id}/graph")
def get_memory_graph(
    memory_id: str,
    workspace_id: Optional[str] = None,
    depth: Optional[int] = None,
    relation_types: Optional[str] = Query(None, description="comma-separated relation_type filter"),
    include_entities: bool = True,
    include_tags: bool = True,
    include_resources: bool = True,
    max_nodes: int = 300,
    format: str = "json",
    memory_service: MemoryService = Depends(get_memory_service),
    graph_service: MemoryGraphService = Depends(get_memory_graph_service),
):
    memory_service.get(memory_id, workspace_id=workspace_id)  # 404s before doing any traversal work
    graph = graph_service.build_graph(
        [memory_id],
        depth=depth,
        relation_types=relation_types.split(",") if relation_types else None,
        include_entities=include_entities,
        include_tags=include_tags,
        include_resources=include_resources,
        max_nodes=max_nodes,
        workspace_id=workspace_id,
    )

    if format == "json":
        return DataResponse(data=MemoryGraphOut.model_validate(graph))
    if format in _TEXT_MEDIA_TYPE:
        text = render_d2(graph) if format == "d2" else render_mermaid(graph)
        return PlainTextResponse(content=text, media_type=_TEXT_MEDIA_TYPE[format])
    if format == "svg":
        svg = render_svg(compute_layout(graph), graph)
        return Response(content=svg, media_type="image/svg+xml")
    if format == "png":
        svg = render_svg(compute_layout(graph), graph)
        return Response(content=render_png(svg), media_type="image/png")

    raise UnsupportedRenderFormatError(f"unsupported format: {format!r}")
