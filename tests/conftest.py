from __future__ import annotations

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session, sessionmaker
from sqlalchemy.pool import StaticPool

from lib.dal.local.database import Base
from lib.dal.remote.document_store import InMemoryMemoryDocumentStore
from lib.dal.repositories.entity_repository import EntityRepository
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.dal.repositories.resource_repository import ResourceRepository
from lib.dal.repositories.tag_repository import TagRepository
from lib.domain.services.embedding import NullEmbeddingProvider
from lib.domain.services.memory_service import MemoryService
from lib.domain.services.recall_service import RecallService


@pytest.fixture()
def session_factory():
    """Fresh in-memory SQLite database per test — fast, isolated, no shared
    state between tests (spec Part 7 §113)."""
    # StaticPool keeps one connection alive for the engine's lifetime: without
    # it, SQLite's `:memory:` database is connection-local, and the API tests
    # execute route handlers in a separate worker thread (FastAPI runs sync
    # endpoints via anyio.to_thread) that would otherwise see a blank database.
    engine = create_engine(
        "sqlite:///:memory:", connect_args={"check_same_thread": False}, poolclass=StaticPool, future=True
    )
    Base.metadata.create_all(engine)
    factory = sessionmaker(bind=engine, autoflush=False, autocommit=False, expire_on_commit=False, class_=Session)
    yield factory
    engine.dispose()


@pytest.fixture()
def memory_repo(session_factory):
    return MemoryRepository(session_factory=session_factory)


@pytest.fixture()
def tag_repo(session_factory):
    return TagRepository(session_factory=session_factory)


@pytest.fixture()
def entity_repo(session_factory):
    return EntityRepository(session_factory=session_factory)


@pytest.fixture()
def resource_repo(session_factory):
    return ResourceRepository(session_factory=session_factory)


@pytest.fixture()
def document_store():
    return InMemoryMemoryDocumentStore()


@pytest.fixture()
def memory_service(memory_repo, tag_repo, entity_repo, resource_repo, document_store):
    return MemoryService(
        memory_repo=memory_repo, tag_repo=tag_repo, entity_repo=entity_repo,
        resource_repo=resource_repo, document_store=document_store,
    )


@pytest.fixture()
def recall_service(memory_repo, tag_repo):
    return RecallService(memory_repo=memory_repo, tag_repo=tag_repo, embedding_provider=NullEmbeddingProvider())
