from __future__ import annotations

from xml.sax.saxutils import escape as _xml_escape

from lib.domain.graph_models import MemoryGraph
from lib.domain.rendering.layout import LayoutResult

_MARGIN = 40.0
_FONT_SIZE = 12

# One fill color per node_type — kept flat/high-contrast rather than a full
# palette system, this is meant to read like a plain D2/dot diagram, not a
# branded dashboard.
_NODE_STYLE = {
    "memory": {"fill": "#e6f0ff", "stroke": "#2563eb"},
    "entity": {"fill": "#eafbea", "stroke": "#16a34a"},
    "tag": {"fill": "#fff4e0", "stroke": "#d97706"},
    "resource": {"fill": "#f3e8ff", "stroke": "#7c3aed"},
    "cluster": {"fill": "#f1f5f9", "stroke": "#64748b"},
}
_DEFAULT_STYLE = {"fill": "#f1f5f9", "stroke": "#334155"}

_EDGE_STYLE = {
    "tagged_with": "#d97706",
    "mentions": "#16a34a",
    "member_of_cluster": "#64748b",
}
_DEFAULT_EDGE_COLOR = "#2563eb"


def _node_shape(x: float, y: float, w: float, h: float, node_type: str, style: dict) -> str:
    fill, stroke = style["fill"], style["stroke"]
    left, top = x - w / 2.0, y - h / 2.0

    if node_type == "entity":
        return (
            f'<ellipse cx="{x}" cy="{y}" rx="{w / 2.0}" ry="{h / 2.0}" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>'
        )
    if node_type == "tag":
        points = f"{x},{top} {x + w / 2.0},{y} {x},{top + h} {x - w / 2.0},{y}"
        return f'<polygon points="{points}" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>'
    if node_type == "resource":
        cut = min(w * 0.15, 20)
        points = (
            f"{left + cut},{top} {x + w / 2.0 - cut},{top} {x + w / 2.0},{y} "
            f"{x + w / 2.0 - cut},{top + h} {left + cut},{top + h} {left},{y}"
        )
        return f'<polygon points="{points}" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>'

    dash = ' stroke-dasharray="6,4"' if node_type == "cluster" else ""
    return (
        f'<rect x="{left}" y="{top}" width="{w}" height="{h}" rx="8" ry="8" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="1.5"{dash}/>'
    )


def render_svg(layout: LayoutResult, graph: MemoryGraph) -> str:
    """Hand-rolled SVG (no rendering dependency beyond string building):
    one shape per node — rounded rect (memory), ellipse (entity), diamond
    (tag), hexagon (resource), dashed rect (cluster) — plus a straight,
    arrow-terminated line per edge, colored by `edge_type`."""
    if not layout.positions:
        return (
            '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80">'
            '<text x="10" y="40" font-family="sans-serif" font-size="12">empty graph</text></svg>'
        )

    max_x = max(p.x + p.w / 2.0 for p in layout.positions.values())
    max_y = max(p.y + p.h / 2.0 for p in layout.positions.values())
    width = max_x + 2 * _MARGIN
    height = max_y + 2 * _MARGIN

    parts: list[str] = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="sans-serif">',
        "<defs>",
        '<marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
        'orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#475569"/></marker>',
        "</defs>",
        f'<g transform="translate({_MARGIN},{_MARGIN})">',
    ]

    for edge in graph.edges:
        source = layout.positions.get(edge.source_id)
        target = layout.positions.get(edge.target_id)
        if source is None or target is None:
            continue
        color = _EDGE_STYLE.get(edge.edge_type, _DEFAULT_EDGE_COLOR)
        parts.append(
            f'<line x1="{source.x}" y1="{source.y}" x2="{target.x}" y2="{target.y}" '
            f'stroke="{color}" stroke-width="1.5" marker-end="url(#arrow)"/>'
        )

    nodes_by_id = {node.id: node for node in graph.nodes}
    for node_id, position in layout.positions.items():
        node = nodes_by_id.get(node_id)
        node_type = node.node_type if node else "memory"
        style = _NODE_STYLE.get(node_type, _DEFAULT_STYLE)
        parts.append(_node_shape(position.x, position.y, position.w, position.h, node_type, style))
        if node is not None:
            parts.append(
                f'<text x="{position.x}" y="{position.y}" font-size="{_FONT_SIZE}" '
                f'text-anchor="middle" dominant-baseline="middle" fill="#0f172a">'
                f"{_xml_escape(node.label)}</text>"
            )

    parts.append("</g></svg>")
    return "".join(parts)
