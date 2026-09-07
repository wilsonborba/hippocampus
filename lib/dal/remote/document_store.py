from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Any, Optional

import httpx

from lib.core.logs import get_logger
from lib.core.settings import Settings, get_settings

logger = get_logger(__name__)


class MemoryDocumentStore(ABC):
    """Durable flexible MemoryDocument content (spec Part 1 §9, Part 3 §44-52).
    CouchDB is the intended backend; when it's unset or unreachable, callers
    degrade to structured-only memories rather than fail (spec Part 6 §36,
    "PostgreSQL remains authoritative" — a missing document never means a
    missing Memory)."""

    @abstractmethod
    def put(self, memory_id: str, document: dict[str, Any]) -> bool: ...

    @abstractmethod
    def get(self, memory_id: str) -> Optional[dict[str, Any]]: ...

    @abstractmethod
    def delete(self, memory_id: str) -> bool: ...

    @property
    @abstractmethod
    def available(self) -> bool: ...


class InMemoryMemoryDocumentStore(MemoryDocumentStore):
    """Default when CouchDB is unset or unreachable. See CouchDBMemoryDocumentStore
    for the real adapter."""

    def __init__(self) -> None:
        self._docs: dict[str, dict[str, Any]] = {}

    def put(self, memory_id: str, document: dict[str, Any]) -> bool:
        self._docs[memory_id] = document
        return True

    def get(self, memory_id: str) -> Optional[dict[str, Any]]:
        return self._docs.get(memory_id)

    def delete(self, memory_id: str) -> bool:
        return self._docs.pop(memory_id, None) is not None

    @property
    def available(self) -> bool:
        return True


class CouchDBMemoryDocumentStore(MemoryDocumentStore):
    """Talks to CouchDB's plain REST API directly (PUT/GET/DELETE per doc id) —
    no SDK dependency needed for something this small (spec Part 3 §44-48).
    `_rev` handling stays internal to this adapter (spec Part 3 §47): callers
    never see or manage CouchDB revisions."""

    def __init__(
        self, base_url: str, database: str, auth: Optional[tuple[str, str]] = None, timeout: float = 10.0
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._database = database
        self._auth = auth
        self._timeout = timeout

    def _doc_url(self, memory_id: str) -> str:
        return f"{self._base_url}/{self._database}/memory:{memory_id}"

    def put(self, memory_id: str, document: dict[str, Any]) -> bool:
        try:
            with httpx.Client(timeout=self._timeout, auth=self._auth) as client:
                existing_rev = self._get_rev(client, memory_id)
                payload = dict(document)
                payload["_id"] = f"memory:{memory_id}"
                payload["memory_id"] = memory_id
                if existing_rev:
                    payload["_rev"] = existing_rev
                response = client.put(self._doc_url(memory_id), json=payload)
                response.raise_for_status()
            return True
        except httpx.HTTPError as exc:
            logger.warning("CouchDB put failed for memory_id=%r: %s", memory_id, exc)
            return False

    def get(self, memory_id: str) -> Optional[dict[str, Any]]:
        try:
            with httpx.Client(timeout=self._timeout, auth=self._auth) as client:
                response = client.get(self._doc_url(memory_id))
                if response.status_code == 404:
                    return None
                response.raise_for_status()
                return response.json()
        except httpx.HTTPError as exc:
            logger.warning("CouchDB get failed for memory_id=%r: %s", memory_id, exc)
            return None

    def delete(self, memory_id: str) -> bool:
        try:
            with httpx.Client(timeout=self._timeout, auth=self._auth) as client:
                rev = self._get_rev(client, memory_id)
                if rev is None:
                    return False
                response = client.delete(self._doc_url(memory_id), params={"rev": rev})
                response.raise_for_status()
            return True
        except httpx.HTTPError as exc:
            logger.warning("CouchDB delete failed for memory_id=%r: %s", memory_id, exc)
            return False

    def _get_rev(self, client: httpx.Client, memory_id: str) -> Optional[str]:
        response = client.head(self._doc_url(memory_id))
        if response.status_code == 404:
            return None
        etag = response.headers.get("ETag")
        return etag.strip('"') if etag else None

    @property
    def available(self) -> bool:
        try:
            with httpx.Client(timeout=self._timeout, auth=self._auth) as client:
                response = client.get(f"{self._base_url}/{self._database}")
                return response.status_code == 200
        except httpx.HTTPError:
            return False


def build_document_store(settings: Optional[Settings] = None) -> MemoryDocumentStore:
    settings = settings or get_settings()
    if not settings.couchdb_url:
        return InMemoryMemoryDocumentStore()
    auth = None
    if settings.couchdb_username and settings.couchdb_password:
        auth = (settings.couchdb_username, settings.couchdb_password)
    store = CouchDBMemoryDocumentStore(
        base_url=settings.couchdb_url, database=settings.couchdb_database, auth=auth
    )
    if not store.available:
        logger.warning("CouchDB unavailable at %s; using in-memory document store", settings.couchdb_url)
        return InMemoryMemoryDocumentStore()
    return store
