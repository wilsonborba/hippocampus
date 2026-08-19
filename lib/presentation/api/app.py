from __future__ import annotations

from contextlib import asynccontextmanager
from typing import AsyncIterator, Optional

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from lib.core.logs import LogTarget, configure_logging, get_logger
from lib.core.settings import Settings, get_settings
from lib.dal.migrations import upgrade_db
from lib.domain.errors import DomainError
from lib.presentation.api.routes import (
    entities,
    health,
    memories,
    recall,
    resources,
    search,
    tags,
    working,
)

logger = get_logger(__name__)


def _build_lifespan(settings: Settings):
    @asynccontextmanager
    async def _lifespan(app: FastAPI) -> AsyncIterator[None]:
        configure_logging(target=LogTarget.API, log_file=settings.log_file)
        try:
            upgrade_db(settings.database_url)
        except Exception:
            logger.warning("DB migration check failed on startup", exc_info=True)
        yield

    return _lifespan


def create_app(settings: Optional[Settings] = None) -> FastAPI:
    settings = settings or get_settings()
    app = FastAPI(title="Hippocampus", version="0.1.0", lifespan=_build_lifespan(settings))

    app.add_middleware(
        CORSMiddleware,
        allow_origins=settings.cors_allow_origins,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    @app.exception_handler(DomainError)
    async def _domain_error(request: Request, exc: DomainError) -> JSONResponse:
        return JSONResponse(
            status_code=exc.status_code,
            content={"error": {"code": exc.code, "message": str(exc), "details": {}}},
        )

    @app.exception_handler(Exception)
    async def _unhandled(request: Request, exc: Exception) -> JSONResponse:
        logger.error("unhandled error on %s %s", request.method, request.url.path, exc_info=True)
        return JSONResponse(
            status_code=500,
            content={"error": {"code": "internal_error", "message": str(exc), "details": {}}},
        )

    app.include_router(health.router)
    app.include_router(memories.router)
    app.include_router(search.router)
    app.include_router(recall.router)
    app.include_router(tags.router)
    app.include_router(entities.router)
    app.include_router(resources.router)
    app.include_router(working.router)

    return app
