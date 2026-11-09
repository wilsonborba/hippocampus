from __future__ import annotations

from lib.domain.graph_models import GraphEdge, GraphNode, MemoryGraph
from lib.domain.rendering import compute_layout, render_d2, render_mermaid, render_png, render_svg


def _sample_graph() -> MemoryGraph:
    return MemoryGraph(
        nodes=[
            GraphNode(id="m1", node_type="memory", label="First memory"),
            GraphNode(id="m2", node_type="memory", label="Second memory"),
            GraphNode(id="tag:1", node_type="tag", label="project:x"),
        ],
        edges=[
            GraphEdge(source_id="m1", target_id="m2", edge_type="related_to"),
            GraphEdge(source_id="m1", target_id="tag:1", edge_type="tagged_with"),
        ],
        root_ids=["m1"],
    )


def test_render_d2_contains_all_nodes_and_edges():
    text = render_d2(_sample_graph())
    assert '"m1": "First memory"' in text
    assert '"m2": "Second memory"' in text
    assert '"m1" -> "m2": related_to' in text
    assert '"m1" -> "tag:1": tagged_with' in text


def test_render_mermaid_contains_all_nodes_and_edges():
    text = render_mermaid(_sample_graph())
    assert text.startswith("graph TD")
    assert "First memory" in text
    assert "Second memory" in text
    assert "-->|related_to|" in text
    assert "-->|tagged_with|" in text


def test_compute_layout_positions_every_node_distinctly():
    graph = _sample_graph()
    layout = compute_layout(graph)
    assert set(layout.positions) == {"m1", "m2", "tag:1"}
    coords = {(p.x, p.y) for p in layout.positions.values()}
    assert len(coords) == 3  # no two nodes collapsed onto the same point


def test_compute_layout_handles_empty_graph():
    layout = compute_layout(MemoryGraph())
    assert layout.positions == {}


def test_render_svg_has_one_shape_per_node():
    graph = _sample_graph()
    svg = render_svg(compute_layout(graph), graph)
    assert svg.startswith("<svg")
    assert svg.count("<rect") == 2  # the two memory nodes
    assert svg.count("<polygon") == 1  # the tag (diamond)
    assert "First memory" in svg
    assert "Second memory" in svg


def test_render_svg_empty_graph_is_still_valid_svg():
    svg = render_svg(compute_layout(MemoryGraph()), MemoryGraph())
    assert svg.startswith("<svg")


def test_render_png_produces_valid_png_bytes():
    graph = _sample_graph()
    svg = render_svg(compute_layout(graph), graph)
    png_bytes = render_png(svg)
    assert png_bytes[:8] == b"\x89PNG\r\n\x1a\n"
