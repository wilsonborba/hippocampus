from __future__ import annotations

import json
import time
from abc import ABC, abstractmethod
from typing import Any, Optional

from lib.core.logs import get_logger
from lib.core.settings import Settings, get_settings

logger = get_logger(__name__)


class WorkingMemoryStore(ABC):
    """Transient, disposable session context (spec Part 1 §8, Part 6 §41-44).
    Redis is the intended backend, but nothing in the domain layer may treat
    this store as durable — see the in-memory fallback below, which exists
    precisely because Redis absence must degrade a feature, not break one."""

    @abstractmethod
    def get(self, session_id: str) -> Optional[dict[str, Any]]: ...

    @abstractmethod
    def put(self, session_id: str, data: dict[str, Any], ttl_seconds: Optional[int] = None) -> None: ...

    @abstractmethod
    def delete(self, session_id: str) -> None: ...


class InMemoryWorkingMemoryStore(WorkingMemoryStore):
    """Default when Redis is unset or unreachable. Process-local and lost on
    restart — an acceptable trade-off since working memory is defined as
    disposable (spec Part 1 §8: "Redis may be completely emptied without
    permanently destroying authoritative long-term memory"); this store
    simply makes that same guarantee without requiring Redis at all."""

    def __init__(self) -> None:
        self._data: dict[str, tuple[dict[str, Any], Optional[float]]] = {}

    def get(self, session_id: str) -> Optional[dict[str, Any]]:
        entry = self._data.get(session_id)
        if entry is None:
            return None
        value, expires_at = entry
        if expires_at is not None and time.time() > expires_at:
            del self._data[session_id]
            return None
        return value

    def put(self, session_id: str, data: dict[str, Any], ttl_seconds: Optional[int] = None) -> None:
        expires_at = time.time() + ttl_seconds if ttl_seconds else None
        self._data[session_id] = (data, expires_at)

    def delete(self, session_id: str) -> None:
        self._data.pop(session_id, None)


class RedisWorkingMemoryStore(WorkingMemoryStore):
    KEY_PREFIX = "hippocampus:working:"

    def __init__(self, client: Any) -> None:
        self._client = client

    def get(self, session_id: str) -> Optional[dict[str, Any]]:
        raw = self._client.get(f"{self.KEY_PREFIX}{session_id}")
        if raw is None:
            return None
        return json.loads(raw)

    def put(self, session_id: str, data: dict[str, Any], ttl_seconds: Optional[int] = None) -> None:
        self._client.set(f"{self.KEY_PREFIX}{session_id}", json.dumps(data), ex=ttl_seconds)

    def delete(self, session_id: str) -> None:
        self._client.delete(f"{self.KEY_PREFIX}{session_id}")


def build_working_memory_store(settings: Optional[Settings] = None) -> WorkingMemoryStore:
    settings = settings or get_settings()
    if not settings.redis_url:
        return InMemoryWorkingMemoryStore()
    try:
        import redis  # optional dependency, see pyproject `redis` extra

        client = redis.from_url(settings.redis_url, decode_responses=True)
        client.ping()
        return RedisWorkingMemoryStore(client)
    except Exception as exc:  # pragma: no cover - depends on external Redis
        logger.warning("Redis unavailable (%s); using in-memory working memory store", exc)
        return InMemoryWorkingMemoryStore()
