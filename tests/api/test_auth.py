from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from lib.core.settings import Settings
from lib.presentation.api import deps
from lib.presentation.api.app import create_app


@pytest.fixture()
def authenticated_client(memory_repo, tag_repo, entity_repo, resource_repo, document_store, memory_service, recall_service):
    """Same isolated repositories as the default `client` fixture, but with
    API keys configured — exercising the "auth enabled" path instead of the
    open dev-mode default the other API tests rely on."""
    settings = Settings(
        database_url="sqlite:///:memory:",
        api_keys={"cortex": "cortex-secret", "admin-cli": "admin-secret"},
        admin_services=["admin-cli"],
    )
    app = create_app(settings=settings)
    app.dependency_overrides[deps.get_memory_repo] = lambda: memory_repo
    app.dependency_overrides[deps.get_tag_repo] = lambda: tag_repo
    app.dependency_overrides[deps.get_entity_repo] = lambda: entity_repo
    app.dependency_overrides[deps.get_resource_repo] = lambda: resource_repo
    app.dependency_overrides[deps.get_document_store] = lambda: document_store
    app.dependency_overrides[deps.get_memory_service] = lambda: memory_service
    app.dependency_overrides[deps.get_recall_service] = lambda: recall_service
    with TestClient(app) as test_client:
        yield test_client


def test_dev_mode_stays_open_when_no_keys_configured(client):
    """`client` (tests/api/conftest.py) uses default Settings — no
    HIPPOCAMPUS_API_KEYS — so every route must remain reachable without an
    Authorization header, exactly like before auth existed."""
    response = client.post("/api/v1/memories", json={"content": "x"})
    assert response.status_code == 201


def test_missing_authorization_header_is_rejected(authenticated_client):
    response = authenticated_client.post("/api/v1/memories", json={"content": "x"})
    assert response.status_code == 401
    assert response.json()["error"]["code"] == "unauthorized"


def test_invalid_key_is_rejected(authenticated_client):
    response = authenticated_client.post(
        "/api/v1/memories", json={"content": "x"}, headers={"Authorization": "Bearer wrong-key"}
    )
    assert response.status_code == 401


def test_valid_key_is_accepted(authenticated_client):
    response = authenticated_client.post(
        "/api/v1/memories", json={"content": "x"}, headers={"Authorization": "Bearer cortex-secret"}
    )
    assert response.status_code == 201


def test_health_and_ready_stay_unauthenticated(authenticated_client):
    assert authenticated_client.get("/health").status_code == 200
    assert authenticated_client.get("/ready").status_code == 200


def test_hard_delete_requires_admin_scope(authenticated_client):
    memory_id = authenticated_client.post(
        "/api/v1/memories", json={"content": "x"}, headers={"Authorization": "Bearer cortex-secret"}
    ).json()["data"]["id"]

    non_admin = authenticated_client.delete(
        f"/api/v1/memories/{memory_id}", headers={"Authorization": "Bearer cortex-secret"}
    )
    assert non_admin.status_code == 403
    assert non_admin.json()["error"]["code"] == "forbidden"

    admin = authenticated_client.delete(
        f"/api/v1/memories/{memory_id}", headers={"Authorization": "Bearer admin-secret"}
    )
    assert admin.status_code == 200
