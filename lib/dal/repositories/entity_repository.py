from __future__ import annotations

from datetime import datetime, timezone
from typing import List, Optional

from sqlalchemy import or_, select
from sqlalchemy.orm import Session

from lib.dal.local.database import SessionLocal, session_scope
from lib.dal.models import Entity, EntityAlias


def normalize_name(value: str) -> str:
    return " ".join(value.strip().lower().split())


class EntityRepository:
    def __init__(self, session_factory=SessionLocal) -> None:
        self._session_factory = session_factory

    def get_by_id(self, entity_id: str, session: Optional[Session] = None) -> Optional[Entity]:
        if session:
            return session.get(Entity, entity_id)
        with session_scope(self._session_factory) as s:
            return s.get(Entity, entity_id)

    def get_or_create(
        self,
        entity_type: str,
        canonical_name: str,
        description: Optional[str] = None,
        session: Optional[Session] = None,
    ) -> Entity:
        normalized = normalize_name(canonical_name)

        def _op(s: Session) -> Entity:
            existing = s.scalar(
                select(Entity).where(
                    Entity.entity_type == entity_type, Entity.normalized_name == normalized
                )
            )
            if existing:
                return existing
            # An alias may already resolve to a canonical entity even when the
            # display name differs (spec Part 2 §47 "Autodroid" / "autodroid").
            alias = s.scalar(
                select(EntityAlias).where(EntityAlias.normalized_alias == normalized)
            )
            if alias:
                resolved = s.get(Entity, alias.entity_id)
                if resolved and resolved.entity_type == entity_type:
                    return resolved
            entity = Entity(
                entity_type=entity_type,
                canonical_name=canonical_name,
                normalized_name=normalized,
                description=description,
            )
            s.add(entity)
            s.flush()
            return entity

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def add_alias(
        self,
        entity_id: str,
        alias: str,
        source: Optional[str] = None,
        confidence: Optional[float] = None,
        session: Optional[Session] = None,
    ) -> EntityAlias:
        normalized = normalize_name(alias)

        def _op(s: Session) -> EntityAlias:
            existing = s.scalar(
                select(EntityAlias).where(
                    EntityAlias.entity_id == entity_id, EntityAlias.normalized_alias == normalized
                )
            )
            if existing:
                return existing
            row = EntityAlias(
                entity_id=entity_id,
                alias=alias,
                normalized_alias=normalized,
                source=source,
                confidence=confidence,
                created_at=datetime.now(timezone.utc),
            )
            s.add(row)
            s.flush()
            return row

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def search(self, query: str, limit: int = 20, session: Optional[Session] = None) -> List[Entity]:
        like = f"%{normalize_name(query)}%"

        def _op(s: Session) -> List[Entity]:
            stmt = select(Entity).where(
                or_(Entity.normalized_name.like(like), Entity.canonical_name.like(like))
            ).limit(limit)
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)
