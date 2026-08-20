from __future__ import annotations

import cairosvg


def render_png(svg: str) -> bytes:
    """Rasterizes an SVG string to PNG bytes via `cairosvg` — a binding to
    the system's libcairo, not a subprocess/external CLI."""
    return cairosvg.svg2png(bytestring=svg.encode("utf-8"))
