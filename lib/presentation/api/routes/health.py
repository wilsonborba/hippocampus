from __future__ import annotations

from fastapi import APIRouter
from sqlalchemy import text

from lib.core.settings import get_settings
from lib.dal.local.database import get_engine
from lib.dal.remote.document_store import build_document_store
from lib.dal.remote.working_memory_store import RedisWorkingMemoryStore, build_working_memory_store

router = APIRouter(tags=["health"])


@router.get("/health")
def health() -> dict:
    """Liveness only (spec Part 7 §67) — no dependency checks, so it never
    blocks on a slow/dead external service."""
    return {"status": "ok"}


@router.get("/ready")
def ready() -> dict:
    """Readiness distinguishes critical (PostgreSQL) from degraded-optional
    (Redis/CouchDB) dependencies (spec Part 7 §68-71)."""
    settings = get_settings()
    dependencies: dict[str, str] = {}

    try:
        with get_engine(settings.database_url).connect() as conn:
            conn.execute(text("SELECT 1"))
        dependencies["postgresql"] = "ok"
        postgres_ok = True
    except Exception:
        dependencies["postgresql"] = "unavailable"
        postgres_ok = False

    if not settings.redis_url:
        dependencies["redis"] = "unconfigured"
    else:
        working_store = build_working_memory_store(settings)
        dependencies["redis"] = "ok" if isinstance(working_store, RedisWorkingMemoryStore) else "degraded"

    if not settings.couchdb_url:
        dependencies["couchdb"] = "unconfigured"
    else:
        document_store = build_document_store(settings)
        dependencies["couchdb"] = "ok" if document_store.available else "degraded"

    dependencies["filestore"] = "ok" if settings.filestore_url else "unconfigured"
    dependencies["embedding_provider"] = settings.embedding_provider

    status = "ready" if postgres_ok else "not_ready"
    return {"status": status, "dependencies": dependencies}
