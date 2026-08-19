from __future__ import annotations

from functools import lru_cache

from lib.dal.remote.document_store import MemoryDocumentStore, build_document_store
from lib.dal.remote.filestore_client import FileStoreClient, build_default_filestore_client
from lib.dal.remote.working_memory_store import WorkingMemoryStore, build_working_memory_store
from lib.dal.repositories.entity_repository import EntityRepository
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.dal.repositories.resource_repository import ResourceRepository
from lib.dal.repositories.tag_repository import TagRepository
from lib.domain.services.consolidation_service import ConsolidationService
from lib.domain.services.embedding import EmbeddingProvider, build_embedding_provider
from lib.domain.services.memory_service import MemoryService
from lib.domain.services.recall_service import RecallService

# One instance per process, same pattern as lib.core.settings.get_settings.
# Overridable per-test via FastAPI's `app.dependency_overrides[get_x] = ...`.


@lru_cache(maxsize=1)
def get_memory_repo() -> MemoryRepository:
    return MemoryRepository()


@lru_cache(maxsize=1)
def get_tag_repo() -> TagRepository:
    return TagRepository()


@lru_cache(maxsize=1)
def get_entity_repo() -> EntityRepository:
    return EntityRepository()


@lru_cache(maxsize=1)
def get_resource_repo() -> ResourceRepository:
    return ResourceRepository()


@lru_cache(maxsize=1)
def get_document_store() -> MemoryDocumentStore:
    return build_document_store()


@lru_cache(maxsize=1)
def get_working_memory_store() -> WorkingMemoryStore:
    return build_working_memory_store()


@lru_cache(maxsize=1)
def get_filestore_client() -> FileStoreClient:
    return build_default_filestore_client()


@lru_cache(maxsize=1)
def get_embedding_provider() -> EmbeddingProvider:
    return build_embedding_provider()


@lru_cache(maxsize=1)
def get_memory_service() -> MemoryService:
    return MemoryService(
        memory_repo=get_memory_repo(),
        tag_repo=get_tag_repo(),
        entity_repo=get_entity_repo(),
        resource_repo=get_resource_repo(),
        document_store=get_document_store(),
    )


@lru_cache(maxsize=1)
def get_recall_service() -> RecallService:
    return RecallService(
        memory_repo=get_memory_repo(), tag_repo=get_tag_repo(),
        embedding_provider=get_embedding_provider(),
    )


@lru_cache(maxsize=1)
def get_consolidation_service() -> ConsolidationService:
    return ConsolidationService(memory_repo=get_memory_repo())
