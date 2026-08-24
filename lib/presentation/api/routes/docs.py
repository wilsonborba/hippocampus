from __future__ import annotations

from fastapi import APIRouter
from fastapi.responses import HTMLResponse

router = APIRouter(tags=["docs"])

_SCALAR_HTML = """<!doctype html>
<html>
  <head>
    <title>Hippocampus API Reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <script id="api-reference" data-url="/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>
"""


@router.get("/docs", response_class=HTMLResponse, include_in_schema=False)
@router.get("/docs/scalar", response_class=HTMLResponse, include_in_schema=False)
def scalar_docs() -> HTMLResponse:
    """Interactive API reference generated from FastAPI's own OpenAPI
    schema (spec Part 5 §118 — "if the stack supports OpenAPI naturally,
    use OpenAPI"; do not hand-maintain a divergent spec). No build step and
    no Python dependency: Scalar's embed script renders client-side from
    `/openapi.json`, which stays authoritative on its own."""
    return HTMLResponse(_SCALAR_HTML)
