from __future__ import annotations


def test_create_and_get_memory(client):
    response = client.post(
        "/api/v1/memories", json={"content": "CouchDB is the selected document store.", "memory_type": "decision"}
    )
    assert response.status_code == 201
    memory_id = response.json()["data"]["id"]

    response = client.get(f"/api/v1/memories/{memory_id}")
    assert response.status_code == 200
    assert response.json()["data"]["content"] == "CouchDB is the selected document store."


def test_get_missing_memory_returns_404_with_machine_readable_error(client):
    response = client.get("/api/v1/memories/does-not-exist")
    assert response.status_code == 404
    body = response.json()
    assert body["error"]["code"] == "memory_not_found"


def test_create_memory_without_content_returns_400(client):
    response = client.post("/api/v1/memories", json={})
    assert response.status_code == 400
    assert response.json()["error"]["code"] == "invalid_request"


def test_tag_and_untag_lifecycle(client):
    memory_id = client.post("/api/v1/memories", json={"content": "x"}).json()["data"]["id"]

    response = client.post(f"/api/v1/memories/{memory_id}/tags", json={"tags": ["context:work"]})
    assert response.status_code == 200
    tag_id = response.json()["data"][0]["id"]

    response = client.delete(f"/api/v1/memories/{memory_id}/tags/{tag_id}")
    assert response.status_code == 204


def test_supersede_via_api(client):
    old_id = client.post("/api/v1/memories", json={"content": "MongoDB selected."}).json()["data"]["id"]
    new_id = client.post("/api/v1/memories", json={"content": "CouchDB selected."}).json()["data"]["id"]

    response = client.post(f"/api/v1/memories/{old_id}/supersede", json={"new_memory_id": new_id})
    assert response.status_code == 200

    old_memory = client.get(f"/api/v1/memories/{old_id}").json()["data"]
    assert old_memory["status"] == "superseded"


def test_self_relationship_returns_422(client):
    memory_id = client.post("/api/v1/memories", json={"content": "x"}).json()["data"]["id"]
    response = client.post(
        f"/api/v1/memories/{memory_id}/relationships",
        json={"target_memory_id": memory_id, "relation_type": "related_to"},
    )
    assert response.status_code == 422


def test_recall_endpoint_returns_structured_evidence(client):
    client.post("/api/v1/memories", json={"content": "Mapper V3 is the selected architecture.", "memory_type": "decision"})
    response = client.post("/api/v1/recall", json={"query": "mapper"})
    assert response.status_code == 200
    data = response.json()["data"]
    assert len(data) == 1
    assert "score" in data[0]
    assert "signals" in data[0]


def test_health_and_ready(client):
    assert client.get("/health").status_code == 200
    ready = client.get("/ready")
    assert ready.status_code == 200
    assert ready.json()["status"] == "ready"
