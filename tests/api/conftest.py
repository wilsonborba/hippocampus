from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from lib.presentation.api import deps
from lib.presentation.api.app import create_app


@pytest.fixture()
def client(memory_repo, tag_repo, entity_repo, resource_repo, document_store, memory_service, recall_service):
    """Wires the FastAPI app to the same isolated in-memory-SQLite repositories
    the domain-layer tests use (see tests/conftest.py), instead of whatever
    HIPPOCAMPUS_DATABASE_URL happens to be configured — API tests must not
    depend on or mutate real local state (spec Part 7 §113)."""
    app = create_app()
    app.dependency_overrides[deps.get_memory_repo] = lambda: memory_repo
    app.dependency_overrides[deps.get_tag_repo] = lambda: tag_repo
    app.dependency_overrides[deps.get_entity_repo] = lambda: entity_repo
    app.dependency_overrides[deps.get_resource_repo] = lambda: resource_repo
    app.dependency_overrides[deps.get_document_store] = lambda: document_store
    app.dependency_overrides[deps.get_memory_service] = lambda: memory_service
    app.dependency_overrides[deps.get_recall_service] = lambda: recall_service
    with TestClient(app) as test_client:
        yield test_client
