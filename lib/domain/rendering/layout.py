from __future__ import annotations

from dataclasses import dataclass

from grandalf.graphs import Edge as _GEdge
from grandalf.graphs import Graph as _GGraph
from grandalf.graphs import Vertex as _GVertex
from grandalf.layouts import SugiyamaLayout, VertexViewer

from lib.domain.graph_models import MemoryGraph

_CHAR_WIDTH = 7.0
_NODE_HEIGHT = 40.0
_NODE_PADDING_X = 24.0
_MIN_NODE_WIDTH = 60.0
_COMPONENT_MARGIN = 60.0


@dataclass
class NodeLayout:
    """Center coordinate (x, y) and box size (w, h) for one node, in an
    arbitrary layout-space unit — the SVG renderer maps this 1:1 to pixels."""

    x: float
    y: float
    w: float
    h: float


@dataclass
class LayoutResult:
    positions: dict[str, NodeLayout]


def _box_size(label: str) -> tuple[float, float]:
    width = max(len(label) * _CHAR_WIDTH + _NODE_PADDING_X, _MIN_NODE_WIDTH)
    return width, _NODE_HEIGHT


def compute_layout(graph: MemoryGraph) -> LayoutResult:
    """Layered/hierarchical layout (the same "boxes and arrows" style `dot`/D2
    use) computed entirely in-process via `grandalf`'s Sugiyama algorithm —
    pure Python, no subprocess/external binary.

    Edge routing is deliberately a straight line between the two already-
    positioned node centers: grandalf's own bent polyline routing
    (`SugiyamaLayout.draw_edges`) only runs for edges that carry a `.view`
    object, which is skipped here to avoid depending on that lesser-exercised
    part of the library. The SVG renderer draws edges itself from
    `LayoutResult.positions`.
    """
    if not graph.nodes:
        return LayoutResult(positions={})

    vertices: dict[str, _GVertex] = {}
    for node in graph.nodes:
        vertex = _GVertex(data=node.id)
        w, h = _box_size(node.label)
        vertex.view = VertexViewer(w=w, h=h)
        vertices[node.id] = vertex

    edges = []
    for edge in graph.edges:
        source = vertices.get(edge.source_id)
        target = vertices.get(edge.target_id)
        if source is None or target is None or source is target:
            continue
        edges.append(_GEdge(source, target))

    g = _GGraph(list(vertices.values()), edges)

    positions: dict[str, NodeLayout] = {}
    x_offset = 0.0
    for component in g.C:
        sug = SugiyamaLayout(component)
        sug.init_all()
        sug.draw()

        comp_vertices = list(component.sV)
        if not comp_vertices:
            continue
        min_x = min(v.view.xy[0] for v in comp_vertices)
        max_x = max(v.view.xy[0] + v.view.w / 2.0 for v in comp_vertices)

        for vertex in comp_vertices:
            if vertex.data is None:
                continue  # a Sugiyama "dummy" vertex inserted for a long edge
            x, y = vertex.view.xy
            positions[vertex.data] = NodeLayout(
                x=x - min_x + x_offset, y=y, w=vertex.view.w, h=vertex.view.h,
            )
        x_offset += (max_x - min_x) + _COMPONENT_MARGIN

    return LayoutResult(positions=positions)
