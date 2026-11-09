from __future__ import annotations

from typing import List, Optional

from sqlalchemy import or_, select
from sqlalchemy.orm import Session

from lib.dal.local.database import SessionLocal, session_scope
from lib.dal.models import Tag


def normalize_tag_part(value: str) -> str:
    """Lowercase/slug normalization (spec Part 2 §38): callers may send
    "Machine Learning" or "machine_learning" and land on the same tag."""
    return "-".join(value.strip().lower().replace("_", " ").split())


class TagRepository:
    def __init__(self, session_factory=SessionLocal) -> None:
        self._session_factory = session_factory

    def get_by_id(self, tag_id: str, session: Optional[Session] = None) -> Optional[Tag]:
        if session:
            return session.get(Tag, tag_id)
        with session_scope(self._session_factory) as s:
            return s.get(Tag, tag_id)

    def get_or_create(
        self, namespace: str, value: str, description: Optional[str] = None,
        session: Optional[Session] = None,
    ) -> Tag:
        namespace_n = normalize_tag_part(namespace)
        value_n = normalize_tag_part(value)

        def _op(s: Session) -> Tag:
            existing = s.scalar(
                select(Tag).where(Tag.namespace == namespace_n, Tag.value == value_n)
            )
            if existing:
                return existing
            tag = Tag(
                namespace=namespace_n,
                value=value_n,
                canonical_name=f"{namespace_n}:{value_n}",
                description=description,
            )
            s.add(tag)
            s.flush()
            return tag

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def parse_and_get_or_create(self, canonical: str, session: Optional[Session] = None) -> Tag:
        """Accepts "namespace:value" (spec Part 2 §37); a bare value with no
        namespace is filed under the "general" namespace rather than rejected,
        keeping tag entry low-friction for callers that don't care about
        namespacing yet."""
        if ":" in canonical:
            namespace, value = canonical.split(":", 1)
        else:
            namespace, value = "general", canonical
        return self.get_or_create(namespace, value, session=session)

    def search(self, query: str, limit: int = 20, session: Optional[Session] = None) -> List[Tag]:
        like = f"%{normalize_tag_part(query)}%"

        def _op(s: Session) -> List[Tag]:
            stmt = (
                select(Tag)
                .where(or_(Tag.canonical_name.like(like), Tag.value.like(like)))
                .limit(limit)
            )
            return list(s.scalars(stmt).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def list_all(self, limit: int = 100, session: Optional[Session] = None) -> List[Tag]:
        def _op(s: Session) -> List[Tag]:
            return list(s.scalars(select(Tag).limit(limit)).all())

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)
