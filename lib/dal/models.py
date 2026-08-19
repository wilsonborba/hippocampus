from __future__ import annotations

import uuid
from datetime import datetime
from enum import Enum
from typing import Any, List, Optional

from sqlalchemy import (
    Boolean,
    CheckConstraint,
    DateTime,
    Float,
    ForeignKey,
    Index,
    Integer,
    JSON,
    String,
    Text,
    UniqueConstraint,
)
from sqlalchemy.orm import Mapped, mapped_column

from lib.dal.local.database import Base, TimestampMixin


def new_id() -> str:
    """Internal stable identity for every durable domain object (spec Part 2 §5,
    Part 3 §7). Plain UUID4 hex: sortable IDs (ULID) are a valid alternative but
    not mandated by the spec, and switching later is a non-breaking migration
    since the column stays a String primary key either way."""
    return uuid.uuid4().hex


# -- controlled vocabularies (spec Part 2 §10, §21; Part 3 §30) -------------------
# Plain string-backed enums, not native DB enum types: portable across
# SQLite (dev/test) and PostgreSQL (production) without dialect-specific
# migrations, matching the project convention (see cortex's AccessStatus).


class MemoryType(str, Enum):
    WORKING = "working"
    EPISODIC = "episodic"
    SEMANTIC = "semantic"
    DECISION = "decision"
    PREFERENCE = "preference"
    OBSERVATION = "observation"
    INSTRUCTION = "instruction"
    CORRECTION = "correction"
    ARTIFACT = "artifact"
    DERIVED = "derived"


class MemoryStatus(str, Enum):
    ACTIVE = "active"
    SUPERSEDED = "superseded"
    ARCHIVED = "archived"
    EXPIRED = "expired"
    FORGOTTEN = "forgotten"
    DELETED = "deleted"


class TagSource(str, Enum):
    USER = "user"
    AGENT = "agent"
    CLASSIFIER = "classifier"
    RULE = "rule"
    IMPORT = "import"
    SYSTEM = "system"


class RelationType(str, Enum):
    RELATED_TO = "related_to"
    DERIVED_FROM = "derived_from"
    SUPPORTS = "supports"
    CONTRADICTS = "contradicts"
    SUPERSEDES = "supersedes"
    CORRECTS = "corrects"
    REFERENCES = "references"
    BELONGS_TO = "belongs_to"
    DISCUSSES = "discusses"
    IMPLEMENTS = "implements"
    DEPENDS_ON = "depends_on"
    CAUSED_BY = "caused_by"
    RESULTED_IN = "resulted_in"
    PART_OF = "part_of"
    ASSOCIATED_WITH = "associated_with"


class ResourceStatus(str, Enum):
    AVAILABLE = "available"
    MISSING = "missing"
    UNREACHABLE = "unreachable"
    DELETED = "deleted"
    UNKNOWN = "unknown"


class ResourceOwnership(str, Enum):
    EXTERNAL = "external"
    HIPPOCAMPUS = "hippocampus"


class ProvenanceSourceType(str, Enum):
    USER = "user"
    CONVERSATION = "conversation"
    MESSAGE = "message"
    DOCUMENT = "document"
    FILE = "file"
    WEBSITE = "website"
    API = "api"
    PLANE = "plane"
    GITHUB = "github"
    CORTEX = "cortex"
    HIPPOCAMPUS = "hippocampus"
    IMPORT = "import"
    RULE = "rule"
    CLASSIFIER = "classifier"
    MANUAL = "manual"


class MemoryEventType(str, Enum):
    CREATED = "created"
    UPDATED = "updated"
    TAGGED = "tagged"
    UNTAGGED = "untagged"
    LINKED = "linked"
    UNLINKED = "unlinked"
    REINFORCED = "reinforced"
    SUPERSEDED = "superseded"
    CORRECTED = "corrected"
    ARCHIVED = "archived"
    FORGOTTEN = "forgotten"
    RESTORED = "restored"
    DELETED = "deleted"
    CONSOLIDATED = "consolidated"


# -- core tables --------------------------------------------------------------------


class Memory(Base, TimestampMixin):
    """The principal domain object (spec Part 2 §3-4, Part 3 §6). PostgreSQL is
    authoritative for this row; rich/flexible content may additionally live in
    a CouchDB MemoryDocument referenced by document_store/document_id."""

    __tablename__ = "memory"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_type: Mapped[str] = mapped_column(String(32), nullable=False, index=True)
    status: Mapped[str] = mapped_column(
        String(32), default=MemoryStatus.ACTIVE.value, nullable=False, index=True
    )

    title: Mapped[Optional[str]] = mapped_column(String(256), nullable=True)
    summary: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    # Small canonical content may live here directly (spec Part 3 §10); larger
    # or schema-flexible content belongs in the CouchDB MemoryDocument instead.
    content: Mapped[Optional[str]] = mapped_column(Text, nullable=True)

    importance: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)

    observed_at: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)
    valid_from: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)
    valid_until: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)
    expires_at: Mapped[Optional[datetime]] = mapped_column(
        DateTime(timezone=True), nullable=True, index=True
    )

    last_accessed_at: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)
    access_count: Mapped[int] = mapped_column(Integer, default=0, nullable=False)
    reinforcement_count: Mapped[int] = mapped_column(Integer, default=0, nullable=False)
    last_reinforced_at: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)

    # Reference to the CouchDB (or other) rich document, not the document itself
    # (spec Part 3 §11). Null when the memory has no rich document.
    document_store: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    document_id: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)

    created_by: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    source_kind: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)

    metadata_json: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)

    deleted_at: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)

    __table_args__ = (
        Index("idx_memory_status_type", "status", "memory_type"),
        Index("idx_memory_valid_range", "valid_from", "valid_until"),
    )


class Tag(Base, TimestampMixin):
    """Global, authoritative classification marker (spec Part 2 §36-44). Tags are
    reusable across Memory and Resource; PostgreSQL owns tag identity."""

    __tablename__ = "tag"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    namespace: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    value: Mapped[str] = mapped_column(String(128), nullable=False, index=True)
    canonical_name: Mapped[str] = mapped_column(String(192), nullable=False, unique=True)
    description: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    status: Mapped[str] = mapped_column(String(32), default="active", nullable=False)

    __table_args__ = (UniqueConstraint("namespace", "value", name="uq_tag_namespace_value"),)


class MemoryTag(Base):
    """memory <-> tag association (spec Part 3 §15). Repeated automatic tagging
    updates/reinforces this row rather than creating a duplicate."""

    __tablename__ = "memory_tag"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    tag_id: Mapped[str] = mapped_column(ForeignKey("tag.id"), nullable=False, index=True)
    source: Mapped[str] = mapped_column(String(32), default=TagSource.USER.value, nullable=False)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    weight: Mapped[float] = mapped_column(Float, default=1.0, nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (UniqueConstraint("memory_id", "tag_id", name="uq_memory_tag"),)


class Entity(Base, TimestampMixin):
    """Stable identifiable concept referenced across memories (spec Part 2 §45-48)."""

    __tablename__ = "entity"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    entity_type: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    canonical_name: Mapped[str] = mapped_column(String(256), nullable=False)
    normalized_name: Mapped[str] = mapped_column(String(256), nullable=False, index=True)
    description: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    status: Mapped[str] = mapped_column(String(32), default="active", nullable=False)
    metadata_json: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)

    __table_args__ = (
        UniqueConstraint("entity_type", "normalized_name", name="uq_entity_type_normalized_name"),
    )


class EntityAlias(Base):
    """Alternate names resolving to one canonical entity (spec Part 2 §47)."""

    __tablename__ = "entity_alias"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    entity_id: Mapped[str] = mapped_column(ForeignKey("entity.id"), nullable=False, index=True)
    alias: Mapped[str] = mapped_column(String(256), nullable=False)
    normalized_alias: Mapped[str] = mapped_column(String(256), nullable=False, index=True)
    source: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (
        UniqueConstraint("entity_id", "normalized_alias", name="uq_entity_alias"),
    )


class MemoryEntity(Base):
    """memory <-> entity association with an optional role (spec Part 3 §20)."""

    __tablename__ = "memory_entity"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    entity_id: Mapped[str] = mapped_column(ForeignKey("entity.id"), nullable=False, index=True)
    role: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    source: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (
        UniqueConstraint("memory_id", "entity_id", "role", name="uq_memory_entity_role"),
    )


class Resource(Base, TimestampMixin):
    """External or independently persisted object a memory may reference (spec
    Part 2 §49-52, Part 3 §21-25). Never holds raw bytes: File Store, Plane,
    GitHub, etc. remain authoritative for the artifact itself."""

    __tablename__ = "resource"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    resource_type: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    source_system: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    external_id: Mapped[Optional[str]] = mapped_column(String(256), nullable=True, index=True)
    uri: Mapped[Optional[str]] = mapped_column(String(1024), nullable=True)
    title: Mapped[Optional[str]] = mapped_column(String(256), nullable=True)
    content_type: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    checksum: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    size_bytes: Mapped[Optional[int]] = mapped_column(Integer, nullable=True)
    ownership: Mapped[str] = mapped_column(
        String(16), default=ResourceOwnership.EXTERNAL.value, nullable=False
    )
    status: Mapped[str] = mapped_column(
        String(16), default=ResourceStatus.UNKNOWN.value, nullable=False
    )
    metadata_json: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)
    last_verified_at: Mapped[Optional[datetime]] = mapped_column(DateTime(timezone=True), nullable=True)

    __table_args__ = (
        Index("idx_resource_source_identity", "source_system", "resource_type", "external_id"),
    )


class MemoryResource(Base):
    """memory <-> resource association (spec Part 3 §26)."""

    __tablename__ = "memory_resource"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    resource_id: Mapped[str] = mapped_column(ForeignKey("resource.id"), nullable=False, index=True)
    relationship: Mapped[str] = mapped_column(String(32), default="references", nullable=False)
    source: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (
        UniqueConstraint("memory_id", "resource_id", "relationship", name="uq_memory_resource"),
    )


class MemoryRelationship(Base):
    """Directional memory <-> memory edge using the controlled RelationType
    vocabulary (spec Part 2 §53-67, Part 3 §27-30)."""

    __tablename__ = "memory_relationship"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    source_memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    target_memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    relation_type: Mapped[str] = mapped_column(String(32), nullable=False, index=True)
    source: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    metadata_json: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (
        UniqueConstraint(
            "source_memory_id", "target_memory_id", "relation_type", name="uq_memory_relationship"
        ),
        CheckConstraint("source_memory_id != target_memory_id", name="ck_relationship_not_self"),
    )


class MemoryProvenance(Base):
    """Where a memory's content/claim originated (spec Part 2 §68-73, Part 3
    §31-33). Append-only: enrichment adds new rows, it never rewrites history."""

    __tablename__ = "memory_provenance"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    source_type: Mapped[str] = mapped_column(String(32), nullable=False)
    source_system: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    source_resource_id: Mapped[Optional[str]] = mapped_column(
        ForeignKey("resource.id"), nullable=True
    )
    source_memory_id: Mapped[Optional[str]] = mapped_column(ForeignKey("memory.id"), nullable=True)
    actor_type: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    actor_id: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    capture_method: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    metadata_json: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)


class MemoryEvent(Base):
    """Durable audit trail for lifecycle mutations (spec Part 3 §34-35, Part 6
    §19-27). Not a full event-sourcing log — the system stays state-oriented;
    these rows exist for audit/debugging/history, not for state replay."""

    __tablename__ = "memory_event"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    event_type: Mapped[str] = mapped_column(String(32), nullable=False, index=True)
    actor_type: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    actor_id: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    correlation_id: Mapped[Optional[str]] = mapped_column(String(64), nullable=True, index=True)
    payload: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict, nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False, index=True)


class MemoryEmbedding(Base):
    """Derived, rebuildable semantic vector (spec Part 3 §36-43). Stored as a
    portable JSON float array rather than a native pgvector column: no
    embedding model/dimension has been chosen yet (deferred per spec Part 7
    §147), and a native `vector` column can't be validated against a real
    pgvector instance from this environment. Once a model is chosen, add a
    migration that introduces a native pgvector column/index and backfills
    from this table — memory identity is unaffected either way (spec Part 3
    Invariant 6)."""

    __tablename__ = "memory_embedding"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=new_id)
    memory_id: Mapped[str] = mapped_column(ForeignKey("memory.id"), nullable=False, index=True)
    model: Mapped[str] = mapped_column(String(128), nullable=False)
    model_version: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    dimensions: Mapped[int] = mapped_column(Integer, nullable=False)
    embedding: Mapped[List[float]] = mapped_column(JSON, nullable=False)
    source_text_hash: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)

    __table_args__ = (
        UniqueConstraint("memory_id", "model", "model_version", name="uq_memory_embedding_model"),
    )
