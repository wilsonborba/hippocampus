from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Iterable, List, Optional, Sequence

from sqlalchemy import and_, or_, select
from sqlalchemy.orm import Session

from lib.dal.local.database import SessionLocal, session_scope
from lib.dal.models import (
    Memory,
    MemoryEmbedding,
    MemoryEntity,
    MemoryEvent,
    MemoryProvenance,
    MemoryRelationship,
    MemoryResource,
    MemoryStatus,
    MemoryTag,
    Tag,
)


def _now() -> datetime:
    return datetime.now(timezone.utc)


class MemoryRepository:
    """Owns the `memory` row and everything keyed by memory_id: tags, entities,
    resources, relationships, provenance, events, embeddings. These are kept
    together (rather than split into one repository per join table) because
    every domain use case in Part 4 of the spec touches several of them in the
    same PostgreSQL transaction — splitting them would just push that
    coordination into every caller instead of removing it."""

    def __init__(self, session_factory=SessionLocal) -> None:
        self._session_factory = session_factory

    # -- memory core ----------------------------------------------------------

    def create(self, memory: Memory, session: Optional[Session] = None) -> Memory:
        def _op(s: Session) -> Memory:
            s.add(memory)
            s.flush()
            return memory

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def get(
        self,
        memory_id: str,
        include_deleted: bool = False,
        workspace_id: Optional[str] = None,
        session: Optional[Session] = None,
    ) -> Optional[Memory]:
        def _op(s: Session) -> Optional[Memory]:
            m = s.get(Memory, memory_id)
            if m is None:
                return None
            if not include_deleted and m.status == MemoryStatus.DELETED.value:
                return None
            # Fails closed, same as a not-found: a memory outside the
            # caller's workspace must never be distinguishable from one
            # that doesn't exist at all (no side-channel confirming its
            # existence), and this is the one choke point every read path
            # (including MemoryGraphService's relationship traversal) goes
            # through, so enforcing it here is what actually stops a graph
            # BFS from walking across workspace boundaries via a
            # relationship edge.
            if workspace_id and m.workspace_id != workspace_id:
                return None
            return m

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list(
        self,
        workspace_id: Optional[str] = None,
        memory_type: Optional[str] = None,
        statuses: Optional[Sequence[str]] = None,
        tags_any: Optional[Sequence[str]] = None,
        tags_all: Optional[Sequence[str]] = None,
        entity_id: Optional[str] = None,
        resource_id: Optional[str] = None,
        text: Optional[str] = None,
        created_after: Optional[datetime] = None,
        created_before: Optional[datetime] = None,
        observed_after: Optional[datetime] = None,
        observed_before: Optional[datetime] = None,
        limit: int = 20,
        offset: int = 0,
        session: Optional[Session] = None,
    ) -> List[Memory]:
        """Deterministic structured search (spec Part 4 §14-16). Explicit
        criteria only — no vector similarity here, that's semantic_search."""

        def _op(s: Session) -> List[Memory]:
            stmt = select(Memory)
            if workspace_id:
                stmt = stmt.where(Memory.workspace_id == workspace_id)
            if memory_type:
                stmt = stmt.where(Memory.memory_type == memory_type)
            if statuses:
                stmt = stmt.where(Memory.status.in_(statuses))
            else:
                stmt = stmt.where(Memory.status != MemoryStatus.DELETED.value)
            if created_after:
                stmt = stmt.where(Memory.created_at >= created_after)
            if created_before:
                stmt = stmt.where(Memory.created_at <= created_before)
            if observed_after:
                stmt = stmt.where(Memory.observed_at >= observed_after)
            if observed_before:
                stmt = stmt.where(Memory.observed_at <= observed_before)
            if text:
                like = f"%{text}%"
                stmt = stmt.where(
                    or_(Memory.title.like(like), Memory.summary.like(like), Memory.content.like(like))
                )
            if entity_id:
                stmt = stmt.where(
                    Memory.id.in_(
                        select(MemoryEntity.memory_id).where(MemoryEntity.entity_id == entity_id)
                    )
                )
            if resource_id:
                stmt = stmt.where(
                    Memory.id.in_(
                        select(MemoryResource.memory_id).where(MemoryResource.resource_id == resource_id)
                    )
                )
            if tags_any:
                stmt = stmt.where(
                    Memory.id.in_(
                        select(MemoryTag.memory_id)
                        .join(Tag, Tag.id == MemoryTag.tag_id)
                        .where(Tag.canonical_name.in_(tags_any))
                    )
                )
            if tags_all:
                for canonical in tags_all:
                    stmt = stmt.where(
                        Memory.id.in_(
                            select(MemoryTag.memory_id)
                            .join(Tag, Tag.id == MemoryTag.tag_id)
                            .where(Tag.canonical_name == canonical)
                        )
                    )
            stmt = stmt.order_by(Memory.created_at.desc()).limit(limit).offset(offset)
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def update_fields(
        self, memory_id: str, fields: dict[str, Any], session: Optional[Session] = None
    ) -> Optional[Memory]:
        """Only safe mutable fields should reach here (spec Part 5 §22) — the
        caller (domain service) is responsible for rejecting core-claim
        mutation; this repository just applies whatever it's given."""

        def _op(s: Session) -> Optional[Memory]:
            m = s.get(Memory, memory_id)
            if m is None:
                return None
            for key, value in fields.items():
                setattr(m, key, value)
            return m

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def set_status(
        self, memory_id: str, status: str, session: Optional[Session] = None
    ) -> Optional[Memory]:
        def _op(s: Session) -> Optional[Memory]:
            m = s.get(Memory, memory_id)
            if m:
                m.status = status
                if status == MemoryStatus.DELETED.value:
                    m.deleted_at = _now()
            return m

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def touch_access(self, memory_id: str, session: Optional[Session] = None) -> None:
        """Recall/get side effect only (spec Part 4 §85): access stats, never
        core content."""

        def _op(s: Session) -> None:
            m = s.get(Memory, memory_id)
            if m:
                m.access_count = (m.access_count or 0) + 1
                m.last_accessed_at = _now()

        if session:
            _op(session)
            return
        with session_scope(self._session_factory) as s:
            _op(s)

    def reinforce(self, memory_id: str, session: Optional[Session] = None) -> Optional[Memory]:
        """Distinct from touch_access (spec Part 4 Invariant 9): being read
        does not make a claim more trustworthy, being reinforced does."""

        def _op(s: Session) -> Optional[Memory]:
            m = s.get(Memory, memory_id)
            if m:
                m.reinforcement_count = (m.reinforcement_count or 0) + 1
                m.last_reinforced_at = _now()
            return m

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- tags -------------------------------------------------------------------

    def add_tag(
        self,
        memory_id: str,
        tag_id: str,
        source: str,
        confidence: Optional[float] = None,
        weight: float = 1.0,
        session: Optional[Session] = None,
    ) -> MemoryTag:
        def _op(s: Session) -> MemoryTag:
            existing = s.scalar(
                select(MemoryTag).where(
                    MemoryTag.memory_id == memory_id, MemoryTag.tag_id == tag_id
                )
            )
            if existing:
                # Repeated automatic tagging reinforces rather than duplicates
                # (spec Part 3 §15).
                existing.source = source
                existing.confidence = confidence
                existing.weight = weight
                return existing
            row = MemoryTag(
                memory_id=memory_id, tag_id=tag_id, source=source,
                confidence=confidence, weight=weight, created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def remove_tag(self, memory_id: str, tag_id: str, session: Optional[Session] = None) -> bool:
        def _op(s: Session) -> bool:
            row = s.scalar(
                select(MemoryTag).where(
                    MemoryTag.memory_id == memory_id, MemoryTag.tag_id == tag_id
                )
            )
            if row is None:
                return False
            s.delete(row)
            return True

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_tags(self, memory_id: str, session: Optional[Session] = None) -> List[Tag]:
        def _op(s: Session) -> List[Tag]:
            stmt = select(Tag).join(MemoryTag, MemoryTag.tag_id == Tag.id).where(
                MemoryTag.memory_id == memory_id
            )
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def tags_for_memories(
        self, memory_ids: Iterable[str], session: Optional[Session] = None
    ) -> dict[str, List[Tag]]:
        """Batched, N+1-avoiding lookup (spec Part 4 §138) for recall/list."""
        ids = list(memory_ids)
        if not ids:
            return {}

        def _op(s: Session) -> dict[str, List[Tag]]:
            stmt = (
                select(MemoryTag.memory_id, Tag)
                .join(Tag, Tag.id == MemoryTag.tag_id)
                .where(MemoryTag.memory_id.in_(ids))
            )
            result: dict[str, List[Tag]] = {mid: [] for mid in ids}
            for memory_id, tag in s.execute(stmt).all():
                result.setdefault(memory_id, []).append(tag)
            return result

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- entities -----------------------------------------------------------------

    def add_entity(
        self,
        memory_id: str,
        entity_id: str,
        role: Optional[str] = None,
        source: Optional[str] = None,
        confidence: Optional[float] = None,
        session: Optional[Session] = None,
    ) -> MemoryEntity:
        def _op(s: Session) -> MemoryEntity:
            existing = s.scalar(
                select(MemoryEntity).where(
                    MemoryEntity.memory_id == memory_id,
                    MemoryEntity.entity_id == entity_id,
                    MemoryEntity.role == role,
                )
            )
            if existing:
                return existing
            row = MemoryEntity(
                memory_id=memory_id, entity_id=entity_id, role=role,
                source=source, confidence=confidence, created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_entity_links(self, memory_id: str, session: Optional[Session] = None) -> List[MemoryEntity]:
        def _op(s: Session) -> List[MemoryEntity]:
            return list(s.scalars(select(MemoryEntity).where(MemoryEntity.memory_id == memory_id)).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def memories_for_entity(
        self, entity_id: str, limit: int = 50, session: Optional[Session] = None
    ) -> List[str]:
        def _op(s: Session) -> List[str]:
            stmt = select(MemoryEntity.memory_id).where(MemoryEntity.entity_id == entity_id).limit(limit)
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- resources ------------------------------------------------------------------

    def add_resource_link(
        self,
        memory_id: str,
        resource_id: str,
        relationship: str = "references",
        source: Optional[str] = None,
        session: Optional[Session] = None,
    ) -> MemoryResource:
        def _op(s: Session) -> MemoryResource:
            existing = s.scalar(
                select(MemoryResource).where(
                    MemoryResource.memory_id == memory_id,
                    MemoryResource.resource_id == resource_id,
                    MemoryResource.relationship == relationship,
                )
            )
            if existing:
                return existing
            row = MemoryResource(
                memory_id=memory_id, resource_id=resource_id, relationship=relationship,
                source=source, created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_resource_links(self, memory_id: str, session: Optional[Session] = None) -> List[MemoryResource]:
        def _op(s: Session) -> List[MemoryResource]:
            return list(
                s.scalars(select(MemoryResource).where(MemoryResource.memory_id == memory_id)).all()
            )

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- relationships ----------------------------------------------------------------

    def create_relationship(
        self,
        source_memory_id: str,
        target_memory_id: str,
        relation_type: str,
        source: Optional[str] = None,
        confidence: Optional[float] = None,
        metadata: Optional[dict[str, Any]] = None,
        session: Optional[Session] = None,
    ) -> MemoryRelationship:
        if source_memory_id == target_memory_id:
            # Mirrors the DB CHECK constraint (spec Part 3 §29) with a domain
            # error instead of a raw database exception (spec Part 5 §131).
            raise ValueError("a memory cannot relate to itself")

        def _op(s: Session) -> MemoryRelationship:
            existing = s.scalar(
                select(MemoryRelationship).where(
                    MemoryRelationship.source_memory_id == source_memory_id,
                    MemoryRelationship.target_memory_id == target_memory_id,
                    MemoryRelationship.relation_type == relation_type,
                )
            )
            if existing:
                return existing
            row = MemoryRelationship(
                source_memory_id=source_memory_id, target_memory_id=target_memory_id,
                relation_type=relation_type, source=source, confidence=confidence,
                metadata_json=metadata or {}, created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def delete_relationship(self, relationship_id: str, session: Optional[Session] = None) -> bool:
        def _op(s: Session) -> bool:
            row = s.get(MemoryRelationship, relationship_id)
            if row is None:
                return False
            s.delete(row)
            return True

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_relationships(
        self, memory_id: str, relation_type: Optional[str] = None, session: Optional[Session] = None
    ) -> List[MemoryRelationship]:
        """Both directions (spec Part 5 §48): a memory's neighbors include
        edges where it's the source and edges where it's the target."""

        def _op(s: Session) -> List[MemoryRelationship]:
            stmt = select(MemoryRelationship).where(
                or_(
                    MemoryRelationship.source_memory_id == memory_id,
                    MemoryRelationship.target_memory_id == memory_id,
                )
            )
            if relation_type:
                stmt = stmt.where(MemoryRelationship.relation_type == relation_type)
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- provenance -----------------------------------------------------------------

    def add_provenance(
        self,
        memory_id: str,
        source_type: str,
        source_system: Optional[str] = None,
        source_resource_id: Optional[str] = None,
        source_memory_id: Optional[str] = None,
        actor_type: Optional[str] = None,
        actor_id: Optional[str] = None,
        capture_method: Optional[str] = None,
        confidence: Optional[float] = None,
        metadata: Optional[dict[str, Any]] = None,
        session: Optional[Session] = None,
    ) -> MemoryProvenance:
        def _op(s: Session) -> MemoryProvenance:
            row = MemoryProvenance(
                memory_id=memory_id, source_type=source_type, source_system=source_system,
                source_resource_id=source_resource_id, source_memory_id=source_memory_id,
                actor_type=actor_type, actor_id=actor_id, capture_method=capture_method,
                confidence=confidence, metadata_json=metadata or {}, created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_provenance(self, memory_id: str, session: Optional[Session] = None) -> List[MemoryProvenance]:
        def _op(s: Session) -> List[MemoryProvenance]:
            return list(
                s.scalars(select(MemoryProvenance).where(MemoryProvenance.memory_id == memory_id)).all()
            )

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- events (audit trail) --------------------------------------------------------

    def add_event(
        self,
        memory_id: str,
        event_type: str,
        actor_type: Optional[str] = None,
        actor_id: Optional[str] = None,
        correlation_id: Optional[str] = None,
        payload: Optional[dict[str, Any]] = None,
        session: Optional[Session] = None,
    ) -> MemoryEvent:
        def _op(s: Session) -> MemoryEvent:
            row = MemoryEvent(
                memory_id=memory_id, event_type=event_type, actor_type=actor_type,
                actor_id=actor_id, correlation_id=correlation_id, payload=payload or {},
                created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_events(self, memory_id: str, session: Optional[Session] = None) -> List[MemoryEvent]:
        def _op(s: Session) -> List[MemoryEvent]:
            stmt = select(MemoryEvent).where(MemoryEvent.memory_id == memory_id).order_by(
                MemoryEvent.created_at.asc()
            )
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    # -- embeddings (derived, rebuildable — spec Part 3 §39) --------------------------

    def upsert_embedding(
        self,
        memory_id: str,
        model: str,
        model_version: Optional[str],
        dimensions: int,
        embedding: List[float],
        source_text_hash: Optional[str] = None,
        session: Optional[Session] = None,
    ) -> MemoryEmbedding:
        def _op(s: Session) -> MemoryEmbedding:
            existing = s.scalar(
                select(MemoryEmbedding).where(
                    MemoryEmbedding.memory_id == memory_id,
                    MemoryEmbedding.model == model,
                    MemoryEmbedding.model_version == model_version,
                )
            )
            if existing:
                existing.dimensions = dimensions
                existing.embedding = embedding
                existing.source_text_hash = source_text_hash
                return existing
            row = MemoryEmbedding(
                memory_id=memory_id, model=model, model_version=model_version,
                dimensions=dimensions, embedding=embedding, source_text_hash=source_text_hash,
                created_at=_now(),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_embeddings(
        self, model: Optional[str] = None, session: Optional[Session] = None
    ) -> List[MemoryEmbedding]:
        """Full scan for brute-force cosine similarity (spec Part 3 §42 —
        acceptable at MVP scale; a native pgvector index is the documented
        upgrade path once embeddings are real, see MemoryEmbedding docstring)."""

        def _op(s: Session) -> List[MemoryEmbedding]:
            stmt = select(MemoryEmbedding)
            if model:
                stmt = stmt.where(MemoryEmbedding.model == model)
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)
