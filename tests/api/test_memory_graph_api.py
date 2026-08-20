from __future__ import annotations


def _create_memory(client, content: str) -> str:
    resp = client.post("/api/v1/memories", json={"content": content})
    assert resp.status_code == 201
    return resp.json()["data"]["id"]


def test_graph_json_default_format(client):
    a = _create_memory(client, "a")
    b = _create_memory(client, "b")
    client.post(f"/api/v1/memories/{a}/relationships", json={"target_memory_id": b, "relation_type": "related_to"})

    resp = client.get(f"/api/v1/memories/{a}/graph")
    assert resp.status_code == 200
    data = resp.json()["data"]
    ids = {n["id"] for n in data["nodes"]}
    assert a in ids and b in ids


def test_graph_d2_format(client):
    a = _create_memory(client, "a")
    resp = client.get(f"/api/v1/memories/{a}/graph", params={"format": "d2"})
    assert resp.status_code == 200
    assert resp.headers["content-type"].startswith("text/vnd.d2")
    assert a in resp.text


def test_graph_mermaid_format(client):
    a = _create_memory(client, "a")
    resp = client.get(f"/api/v1/memories/{a}/graph", params={"format": "mermaid"})
    assert resp.status_code == 200
    assert resp.headers["content-type"].startswith("text/vnd.mermaid")
    assert resp.text.startswith("graph TD")


def test_graph_svg_format(client):
    a = _create_memory(client, "a")
    resp = client.get(f"/api/v1/memories/{a}/graph", params={"format": "svg"})
    assert resp.status_code == 200
    assert resp.headers["content-type"].startswith("image/svg+xml")
    assert resp.text.startswith("<svg")


def test_graph_png_format(client):
    a = _create_memory(client, "a")
    resp = client.get(f"/api/v1/memories/{a}/graph", params={"format": "png"})
    assert resp.status_code == 200
    assert resp.headers["content-type"].startswith("image/png")
    assert resp.content[:8] == b"\x89PNG\r\n\x1a\n"


def test_graph_unsupported_format_returns_400(client):
    a = _create_memory(client, "a")
    resp = client.get(f"/api/v1/memories/{a}/graph", params={"format": "pdf"})
    assert resp.status_code == 400
    assert resp.json()["error"]["code"] == "unsupported_render_format"


def test_graph_unknown_memory_returns_404(client):
    resp = client.get("/api/v1/memories/does-not-exist/graph")
    assert resp.status_code == 404


def test_graph_depth_respected(client):
    a = _create_memory(client, "a")
    b = _create_memory(client, "b")
    c = _create_memory(client, "c")
    client.post(f"/api/v1/memories/{a}/relationships", json={"target_memory_id": b, "relation_type": "related_to"})
    client.post(f"/api/v1/memories/{b}/relationships", json={"target_memory_id": c, "relation_type": "related_to"})

    resp = client.get(f"/api/v1/memories/{a}/graph", params={"depth": 1})
    ids = {n["id"] for n in resp.json()["data"]["nodes"]}
    assert c not in ids
