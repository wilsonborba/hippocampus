from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from typing import Optional

from pydantic import AliasChoices, Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
        # Every field below declares a `validation_alias` (its env var
        # name); without this, pydantic only accepts those aliases as
        # constructor kwargs and silently drops plain-field-name kwargs
        # (extra="ignore") — which broke tests/factories constructing
        # `Settings(database_url=..., api_keys=...)` directly.
        populate_by_name=True,
    )

    # Core environment & database
    environment: str = Field(
        default="development",
        validation_alias=AliasChoices("HIPPOCAMPUS_ENVIRONMENT", "ENVIRONMENT", "ENV"),
    )
    database_url: str = Field(
        default="sqlite:///var/hippocampus.db",
        validation_alias=AliasChoices("HIPPOCAMPUS_DATABASE_URL", "DATABASE_URL"),
    )

    # Redis: transient working memory / cache. Optional dependency — see
    # lib.dal.remote.working_memory_store for the graceful-degradation adapter.
    redis_url: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_REDIS_URL", "REDIS_URL"),
    )

    # CouchDB: durable flexible MemoryDocument storage. Optional dependency —
    # see lib.dal.remote.document_store for the graceful-degradation adapter.
    couchdb_url: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_COUCHDB_URL", "COUCHDB_URL"),
    )
    couchdb_database: str = Field(
        default="hippocampus",
        validation_alias=AliasChoices("HIPPOCAMPUS_COUCHDB_DATABASE", "COUCHDB_DATABASE"),
    )
    couchdb_username: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_COUCHDB_USERNAME", "COUCHDB_USERNAME"),
    )
    couchdb_password: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_COUCHDB_PASSWORD", "COUCHDB_PASSWORD"),
    )

    # File Store: existing local large/raw artifact service (reference only —
    # Hippocampus never duplicates bytes into its own databases).
    filestore_url: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_FILESTORE_URL", "FILESTORE_URL"),
    )
    filestore_timeout_seconds: float = Field(
        default=10.0,
        validation_alias=AliasChoices(
            "HIPPOCAMPUS_FILESTORE_TIMEOUT_SECONDS", "FILESTORE_TIMEOUT_SECONDS"
        ),
    )

    # Embedding provider: optional. "none" keeps semantic search degraded but
    # every deterministic capability (remember/get/list/search/tag/...) fully
    # functional, per the spec's graceful-AI-degradation invariant.
    embedding_provider: str = Field(
        default="none",
        validation_alias=AliasChoices("HIPPOCAMPUS_EMBEDDING_PROVIDER", "EMBEDDING_PROVIDER"),
    )
    embedding_model: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("HIPPOCAMPUS_EMBEDDING_MODEL", "EMBEDDING_MODEL"),
    )
    ollama_base_url: str = Field(
        default="http://localhost:11434",
        validation_alias=AliasChoices("HIPPOCAMPUS_OLLAMA_BASE_URL", "OLLAMA_BASE_URL"),
    )

    # Logging
    log_level: str = Field(
        default="INFO",
        validation_alias=AliasChoices("HIPPOCAMPUS_LOG_LEVEL", "LOG_LEVEL"),
    )
    log_file: Path = Field(
        default=Path("var/hippocampus.log"),
        validation_alias=AliasChoices("HIPPOCAMPUS_LOG_FILE", "LOG_FILE"),
    )

    # Server configuration
    api_host: str = Field(
        default="127.0.0.1",
        validation_alias=AliasChoices("HIPPOCAMPUS_API_HOST", "API_HOST"),
    )
    api_port: int = Field(
        default=8001,
        validation_alias=AliasChoices("HIPPOCAMPUS_API_PORT", "API_PORT"),
    )
    cors_allow_origins: list[str] = Field(
        default_factory=lambda: ["*"],
        validation_alias=AliasChoices("HIPPOCAMPUS_CORS_ALLOW_ORIGINS", "CORS_ALLOW_ORIGINS"),
    )

    # Recall / search tuning (weights are intentionally not hard-coded here —
    # see lib.domain.services.recall_service for the scoring composition)
    recall_default_limit: int = Field(
        default=10,
        validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_DEFAULT_LIMIT", "RECALL_DEFAULT_LIMIT"),
    )
    recall_max_limit: int = Field(
        default=50,
        validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_MAX_LIMIT", "RECALL_MAX_LIMIT"),
    )
    graph_max_depth: int = Field(
        default=2,
        validation_alias=AliasChoices("HIPPOCAMPUS_GRAPH_MAX_DEPTH", "GRAPH_MAX_DEPTH"),
    )

    # Recall ranking weights (spec Part 4 §30-33 — a deliberately simple,
    # inspectable weighted sum; no exact values are mandated by the spec, so
    # these stay configuration rather than hard-coded constants).
    recall_weight_semantic: float = Field(
        default=0.30, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_SEMANTIC")
    )
    recall_weight_lexical: float = Field(
        default=0.15, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_LEXICAL")
    )
    recall_weight_tag: float = Field(
        default=0.20, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_TAG")
    )
    recall_weight_entity: float = Field(
        default=0.15, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_ENTITY")
    )
    recall_weight_importance: float = Field(
        default=0.10, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_IMPORTANCE")
    )
    recall_weight_recency: float = Field(
        default=0.10, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_WEIGHT_RECENCY")
    )
    recall_superseded_penalty: float = Field(
        default=0.5, validation_alias=AliasChoices("HIPPOCAMPUS_RECALL_SUPERSEDED_PENALTY")
    )

    # API authentication (spec Part 7 §4, §10-11). Empty dict = auth disabled
    # (local/dev mode) — "the smallest mechanism consistent with current
    # infrastructure", not OAuth. Maps a service name to its static bearer
    # key, same shape as FSM's FSM_APP_KEYS for ecosystem consistency.
    api_keys: dict[str, str] = Field(
        default_factory=dict, validation_alias=AliasChoices("HIPPOCAMPUS_API_KEYS")
    )
    # Service names (keys of `api_keys`) allowed to call elevated/destructive
    # operations such as hard delete (spec Part 7 §10, §23).
    admin_services: list[str] = Field(
        default_factory=list, validation_alias=AliasChoices("HIPPOCAMPUS_ADMIN_SERVICES")
    )

    # Consolidation (spec Part 5 §64 — candidate sets must always be bounded,
    # never "consolidate everything").
    consolidation_max_candidates: int = Field(
        default=50, validation_alias=AliasChoices("HIPPOCAMPUS_CONSOLIDATION_MAX_CANDIDATES")
    )


@lru_cache(maxsize=1)
def get_settings() -> Settings:
    return Settings()
