from __future__ import annotations


def test_scalar_docs_served_without_auth(client):
    response = client.get("/docs/scalar")
    assert response.status_code == 200
    assert "api-reference" in response.text


def test_openapi_schema_available(client):
    response = client.get("/openapi.json")
    assert response.status_code == 200
    assert response.json()["info"]["title"] == "Hippocampus"
