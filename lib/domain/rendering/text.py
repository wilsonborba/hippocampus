from __future__ import annotations

from lib.domain.graph_models import MemoryGraph

# One shape per node_type, matching D2's/dot's "boxes and arrows" vocabulary
# (spec: this is the whole point of the D2-inspired output — plain,
# recognizable primitives, not custom iconography).
_D2_SHAPE_BY_NODE_TYPE = {
    "memory": "rectangle",
    "entity": "oval",
    "tag": "diamond",
    "resource": "hexagon",
    "cluster": "package",
}

_MERMAID_SHAPE_BY_NODE_TYPE = {
    "memory": ("[", "]"),
    "entity": ("([", "])"),
    "tag": ("{", "}"),
    "resource": ("[[", "]]"),
    "cluster": ("[/", "/]"),
}


def _d2_escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


def render_d2(graph: MemoryGraph) -> str:
    """D2 text syntax for the graph. No dependency needed — this is plain
    string templating over the neutral node/edge structure."""
    lines: list[str] = []
    for node in graph.nodes:
        shape = _D2_SHAPE_BY_NODE_TYPE.get(node.node_type, "rectangle")
        lines.append(f'"{_d2_escape(node.id)}": "{_d2_escape(node.label)}" {{shape: {shape}}}')
    for edge in graph.edges:
        lines.append(f'"{_d2_escape(edge.source_id)}" -> "{_d2_escape(edge.target_id)}": {edge.edge_type}')
    return "\n".join(lines)


def _mermaid_escape(text: str) -> str:
    return text.replace('"', "'")


def render_mermaid(graph: MemoryGraph) -> str:
    node_ids = {node.id: f"n{index}" for index, node in enumerate(graph.nodes)}
    lines = ["graph TD"]
    for node in graph.nodes:
        open_, close = _MERMAID_SHAPE_BY_NODE_TYPE.get(node.node_type, ("[", "]"))
        lines.append(f'{node_ids[node.id]}{open_}"{_mermaid_escape(node.label)}"{close}')
    for edge in graph.edges:
        source = node_ids.get(edge.source_id)
        target = node_ids.get(edge.target_id)
        if source is None or target is None:
            continue
        lines.append(f"{source} -->|{edge.edge_type}| {target}")
    return "\n".join(lines)
