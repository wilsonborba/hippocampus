from __future__ import annotations


def test_consolidation_requires_scope(client):
    response = client.post("/api/v1/consolidation", json={"mode": "analyze"})
    assert response.status_code == 400
    assert response.json()["error"]["code"] == "invalid_request"


def test_consolidation_analyze_reports_exact_duplicate(client):
    client.post("/api/v1/memories", json={"content": "CouchDB is the selected document store.", "tags": ["project:x"]})
    client.post("/api/v1/memories", json={"content": "CouchDB is the selected document store.", "tags": ["project:x"]})

    response = client.post(
        "/api/v1/consolidation", json={"filters": {"tags": ["project:x"]}, "mode": "analyze"}
    )
    assert response.status_code == 200
    data = response.json()["data"]
    assert len(data["candidate_duplicates"]) == 1
    assert data["applied_changes"] == []


def test_consolidation_apply_safe_archives_duplicate(client):
    client.post("/api/v1/memories", json={"content": "dup content", "tags": ["project:y"]})
    client.post("/api/v1/memories", json={"content": "dup content", "tags": ["project:y"]})

    response = client.post(
        "/api/v1/consolidation", json={"filters": {"tags": ["project:y"]}, "mode": "apply_safe"}
    )
    assert response.status_code == 200
    data = response.json()["data"]
    assert len(data["applied_changes"]) == 1
    assert data["applied_changes"][0]["action"] == "archived_duplicate"
