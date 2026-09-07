from __future__ import annotations

from typing import Any, Optional

from sqlalchemy import select
from sqlalchemy.orm import Session

from lib.dal.local.database import SessionLocal, session_scope
from lib.dal.models import Resource, ResourceOwnership, ResourceStatus


class ResourceRepository:
    def __init__(self, session_factory=SessionLocal) -> None:
        self._session_factory = session_factory

    def get_by_id(self, resource_id: str, session: Optional[Session] = None) -> Optional[Resource]:
        if session:
            return session.get(Resource, resource_id)
        with session_scope(self._session_factory) as s:
            return s.get(Resource, resource_id)

    def register(
        self,
        resource_type: str,
        source_system: str,
        external_id: Optional[str] = None,
        uri: Optional[str] = None,
        title: Optional[str] = None,
        content_type: Optional[str] = None,
        checksum: Optional[str] = None,
        size_bytes: Optional[int] = None,
        ownership: str = ResourceOwnership.EXTERNAL.value,
        status: str = ResourceStatus.UNKNOWN.value,
        metadata: Optional[dict[str, Any]] = None,
        session: Optional[Session] = None,
    ) -> Resource:
        """Idempotent registration: stable source identity, not the row's
        physical existence, defines uniqueness (spec Part 3 §22 - do not infer
        uniqueness from filenames)."""

        def _op(s: Session) -> Resource:
            existing = None
            if external_id is not None:
                existing = s.scalar(
                    select(Resource).where(
                        Resource.source_system == source_system,
                        Resource.resource_type == resource_type,
                        Resource.external_id == external_id,
                    )
                )
            elif uri is not None:
                existing = s.scalar(
                    select(Resource).where(
                        Resource.source_system == source_system, Resource.uri == uri
                    )
                )
            if existing:
                return existing
            resource = Resource(
                resource_type=resource_type,
                source_system=source_system,
                external_id=external_id,
                uri=uri,
                title=title,
                content_type=content_type,
                checksum=checksum,
                size_bytes=size_bytes,
                ownership=ownership,
                status=status,
                metadata_json=metadata or {},
            )
            s.add(resource)
            s.flush()
            return resource

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)

    def set_status(
        self, resource_id: str, status: str, session: Optional[Session] = None
    ) -> Optional[Resource]:
        def _op(s: Session) -> Optional[Resource]:
            r = s.get(Resource, resource_id)
            if r:
                r.status = status
            return r

        if session:
            return _op(session)
        with session_scope(self._session_factory) as s:
            return _op(s)
