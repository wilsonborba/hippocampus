from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Optional

from lib.dal.models import (
    Entity,
    Memory,
    MemoryEvent,
    MemoryEventType,
    MemoryProvenance,
    MemoryRelationship,
    MemoryStatus,
    MemoryType,
    Resource,
    ResourceOwnership,
    ResourceStatus,
    Tag,
)
from lib.dal.remote.document_store import MemoryDocumentStore
from lib.dal.remote.filestore_client import FileStoreClient
from lib.dal.repositories.entity_repository import EntityRepository
from lib.dal.repositories.memory_repository import MemoryRepository
from lib.dal.repositories.resource_repository import ResourceRepository
from lib.dal.repositories.tag_repository import TagRepository, normalize_tag_part
from lib.domain.errors import (
    FileStoreNotConfiguredError,
    InvalidMemoryStatusError,
    InvalidRelationshipError,
    MemoryNotFoundError,
    ValidationError,
)
from lib.domain.models import MemoryInput, SearchFilters

# Fields a caller may safely mutate after creation without it counting as
# rewriting the memory's core claim (spec Part 5 §22-23).
_MUTABLE_FIELDS = {"title", "summary", "importance", "confidence", "valid_from", "valid_until",
                    "expires_at", "metadata_json"}

_VALID_MEMORY_TYPES = {t.value for t in MemoryType}
_VALID_RELATION_TYPES = {
    "related_to", "derived_from", "supports", "contradicts", "supersedes", "corrects",
    "references", "belongs_to", "discusses", "implements", "depends_on", "caused_by",
    "resulted_in", "part_of", "associated_with",
}
_MAX_PROVENANCE_FIELD_LENGTHS = {
    "source_type": 32,
    "source_system": 64,
    "source_resource_id": 256,
    "source_memory_id": 64,
    "capture_method": 64,
}
_MAX_ASSOC_FIELD_LENGTHS = {
    "entity.role": 32,
    "entity.source": 32,
    "resource.relationship": 32,
    "resource.source": 32,
}
_MAX_WORKSPACE_ID_LENGTH = 64


def _now() -> datetime:
    return datetime.now(timezone.utc)


def canonicalize_tag_filter(tag: str) -> str:
    if ":" in tag:
        namespace, value = tag.split(":", 1)
    else:
        namespace, value = "general", tag
    return f"{normalize_tag_part(namespace)}:{normalize_tag_part(value)}"


def _validate_max_length(field: str, value: Optional[str], max_length: int) -> None:
    if value is not None and len(value) > max_length:
        raise ValidationError(
            f"{field} must be {max_length} characters or fewer; got {len(value)}"
        )


def normalize_workspace_id(workspace_id: Optional[str]) -> str:
    normalized = normalize_tag_part(workspace_id or "default")
    if not normalized:
        raise ValidationError("workspace_id cannot be blank")
    _validate_max_length("workspace_id", normalized, _MAX_WORKSPACE_ID_LENGTH)
    return normalized


class MemoryService:
    """Deterministic core of `remember`/`recall`'s building blocks (spec Part
    4): every method here is exact/rule-based, no LLM call required (Part 4
    §3, §55, Part 6 §53 acceptance criteria)."""

    def __init__(
        self,
        memory_repo: MemoryRepository,
        tag_repo: TagRepository,
        entity_repo: EntityRepository,
        resource_repo: ResourceRepository,
        document_store: MemoryDocumentStore,
        filestore_client: Optional[FileStoreClient] = None,
    ) -> None:
        self._memories = memory_repo
        self._tags = tag_repo
        self._entities = entity_repo
        self._resources = resource_repo
        self._documents = document_store
        self._filestore = filestore_client

    # -- remember -----------------------------------------------------------------

    def remember(
        self, data: MemoryInput, actor_type: str = "user", actor_id: Optional[str] = None,
        correlation_id: Optional[str] = None,
    ) -> Memory:
        memory_type = data.memory_type or MemoryType.SEMANTIC.value
        if memory_type not in _VALID_MEMORY_TYPES:
            raise ValidationError(f"unknown memory_type: {memory_type!r}")
        if not data.content and not data.summary and not data.title:
            raise ValidationError("at least one of content, summary, or title is required")
        self._validate_associations(data)
        workspace_id = normalize_workspace_id(data.workspace_id)

        memory = Memory(
            workspace_id=workspace_id,
            memory_type=memory_type,
            status=MemoryStatus.ACTIVE.value,
            title=data.title,
            summary=data.summary,
            content=data.content,
            importance=data.importance,
            confidence=data.confidence,
            observed_at=data.observed_at,
            valid_from=data.valid_from,
            valid_until=data.valid_until,
            expires_at=data.expires_at,
            created_by=actor_type,
            metadata_json=data.metadata or {},
        )
        self._memories.create(memory)

        for canonical in data.tags:
            tag = self._tags.parse_and_get_or_create(canonical)
            self._memories.add_tag(memory.id, tag.id, source=actor_type)

        for entity_ref in data.entities:
            entity = self._entities.get_or_create(
                entity_type=entity_ref["entity_type"], canonical_name=entity_ref["canonical_name"],
            )
            self._memories.add_entity(
                memory.id, entity.id, role=entity_ref.get("role"), source=actor_type,
            )

        for resource_ref in data.resources:
            resource = self._resources.register(
                resource_type=resource_ref["resource_type"],
                source_system=resource_ref["source_system"],
                external_id=resource_ref.get("external_id"),
                uri=resource_ref.get("uri"),
                title=resource_ref.get("title"),
            )
            self._memories.add_resource_link(
                memory.id, resource.id, relationship=resource_ref.get("relationship", "references"),
                source=actor_type,
            )

        if data.provenance or actor_type:
            self._memories.add_provenance(
                memory.id,
                source_type=data.provenance.get("source_type", actor_type),
                source_system=data.provenance.get("source_system"),
                actor_type=actor_type,
                actor_id=actor_id,
                capture_method=data.provenance.get("capture_method"),
                confidence=data.provenance.get("confidence"),
                metadata=data.provenance.get("metadata"),
            )

        if data.document:
            self._documents.put(memory.id, data.document)
            self._memories.update_fields(
                memory.id, {"document_store": "couchdb", "document_id": f"memory:{memory.id}"}
            )

        self._memories.add_event(
            memory.id, MemoryEventType.CREATED.value, actor_type=actor_type, actor_id=actor_id,
            correlation_id=correlation_id,
        )
        return self.get(memory.id)

    def _validate_associations(self, data: MemoryInput) -> None:
        provenance = data.provenance or {}
        for field, max_length in _MAX_PROVENANCE_FIELD_LENGTHS.items():
            _validate_max_length(f"provenance.{field}", provenance.get(field), max_length)
        for entity_ref in data.entities:
            _validate_max_length("entities[].role", entity_ref.get("role"), _MAX_ASSOC_FIELD_LENGTHS["entity.role"])
            _validate_max_length("entities[].source", entity_ref.get("source"), _MAX_ASSOC_FIELD_LENGTHS["entity.source"])
        for resource_ref in data.resources:
            _validate_max_length(
                "resources[].relationship",
                resource_ref.get("relationship", "references"),
                _MAX_ASSOC_FIELD_LENGTHS["resource.relationship"],
            )
            _validate_max_length("resources[].source", resource_ref.get("source"), _MAX_ASSOC_FIELD_LENGTHS["resource.source"])

    # -- read -----------------------------------------------------------------------

    def get(self, memory_id: str, touch: bool = False, workspace_id: Optional[str] = None) -> Memory:
        memory = self._memories.get(
            memory_id, workspace_id=normalize_workspace_id(workspace_id) if workspace_id else None
        )
        if memory is None:
            raise MemoryNotFoundError(f"memory {memory_id!r} not found")
        if touch:
            self._memories.touch_access(memory_id)
        return memory

    def list(self, filters: SearchFilters) -> list[Memory]:
        tags_any = [canonicalize_tag_filter(tag) for tag in filters.tags_any]
        tags_all = [canonicalize_tag_filter(tag) for tag in filters.tags_all]
        return self._memories.list(
            workspace_id=normalize_workspace_id(filters.workspace_id) if filters.workspace_id else None,
            memory_type=filters.memory_types[0] if filters.memory_types else None,
            statuses=filters.statuses or None,
            tags_any=tags_any or None,
            tags_all=tags_all or None,
            entity_id=filters.entity_id,
            resource_id=filters.resource_id,
            text=filters.text,
            created_after=filters.created_after,
            created_before=filters.created_before,
            observed_after=filters.observed_after,
            observed_before=filters.observed_before,
            limit=filters.limit,
        )

    def search(self, filters: SearchFilters) -> list[Memory]:
        """Alias kept distinct from `list` at the call site (spec Part 4 §14):
        the same deterministic repository query backs both today, but they
        are conceptually different public operations and may diverge later
        (e.g. search adding lexical ranking `list` doesn't need)."""
        return self.list(filters)

    def tags_for(self, memory_id: str) -> list[Tag]:
        return self._memories.list_tags(memory_id)

    def provenance(self, memory_id: str) -> list[MemoryProvenance]:
        return self._memories.list_provenance(memory_id)

    def events(self, memory_id: str) -> list[MemoryEvent]:
        return self._memories.list_events(memory_id)

    def document(self, memory_id: str) -> Optional[dict[str, Any]]:
        return self._documents.get(memory_id)

    # -- update ---------------------------------------------------------------------

    def update(self, memory_id: str, fields: dict[str, Any]) -> Memory:
        unsafe = set(fields) - _MUTABLE_FIELDS
        if unsafe:
            raise ValidationError(
                f"field(s) {sorted(unsafe)} cannot be updated directly; create a new memory and "
                "correct/supersede this one instead (spec Part 5 §23)"
            )
        memory = self._memories.update_fields(memory_id, fields)
        if memory is None:
            raise MemoryNotFoundError(f"memory {memory_id!r} not found")
        self._memories.add_event(memory_id, MemoryEventType.UPDATED.value, payload={"fields": list(fields)})
        return memory

    # -- tags -------------------------------------------------------------------------

    def tag(self, memory_id: str, tags: list[str], source: str = "user",
            confidence: Optional[float] = None) -> list[Tag]:
        self.get(memory_id)
        _validate_max_length("source", source, _MAX_ASSOC_FIELD_LENGTHS["entity.source"])
        added = []
        for canonical in tags:
            tag_row = self._tags.parse_and_get_or_create(canonical)
            self._memories.add_tag(memory_id, tag_row.id, source=source, confidence=confidence)
            added.append(tag_row)
        self._memories.add_event(
            memory_id, MemoryEventType.TAGGED.value, payload={"tags": [t.canonical_name for t in added]}
        )
        return added

    def untag(self, memory_id: str, tag_id: str) -> bool:
        removed = self._memories.remove_tag(memory_id, tag_id)
        if removed:
            self._memories.add_event(memory_id, MemoryEventType.UNTAGGED.value, payload={"tag_id": tag_id})
        return removed

    # -- entities / resources ---------------------------------------------------------

    def associate_entity(
        self, memory_id: str, entity_type: str, canonical_name: str, role: Optional[str] = None,
        source: Optional[str] = None, confidence: Optional[float] = None,
    ) -> Entity:
        self.get(memory_id)
        _validate_max_length("role", role, _MAX_ASSOC_FIELD_LENGTHS["entity.role"])
        _validate_max_length("source", source, _MAX_ASSOC_FIELD_LENGTHS["entity.source"])
        entity = self._entities.get_or_create(entity_type, canonical_name)
        self._memories.add_entity(memory_id, entity.id, role=role, source=source, confidence=confidence)
        return entity

    def associate_resource(
        self, memory_id: str, resource_type: str, source_system: str,
        external_id: Optional[str] = None, uri: Optional[str] = None,
        relationship: str = "references",
    ) -> Resource:
        self.get(memory_id)
        _validate_max_length("relationship", relationship, _MAX_ASSOC_FIELD_LENGTHS["resource.relationship"])
        resource = self._resources.register(
            resource_type=resource_type, source_system=source_system,
            external_id=external_id, uri=uri,
        )
        self._memories.add_resource_link(memory_id, resource.id, relationship=relationship)
        return resource

    def upload_and_associate_resource(
        self,
        memory_id: str,
        filename: str,
        content: bytes,
        content_type: str,
        relationship: str = "references",
        title: Optional[str] = None,
        source: str = "user",
    ) -> Resource:
        self.get(memory_id)
        _validate_max_length("relationship", relationship, _MAX_ASSOC_FIELD_LENGTHS["resource.relationship"])
        _validate_max_length("source", source, _MAX_ASSOC_FIELD_LENGTHS["resource.source"])
        if not self._filestore or not self._filestore.configured:
            raise FileStoreNotConfiguredError("File Store (FSM) is not configured")

        upload_result = self._filestore.upload(
            album=f"memory-{memory_id}",
            filename=filename,
            body=content,
            content_type=content_type,
        )

        resource = self._resources.register(
            resource_type="file",
            source_system="fsm",
            external_id=upload_result.key,
            uri=upload_result.uri,
            title=title or filename,
            content_type=upload_result.content_type,
            checksum=upload_result.checksum,
            size_bytes=upload_result.size_bytes,
            ownership=ResourceOwnership.HIPPOCAMPUS.value,
            status=ResourceStatus.AVAILABLE.value,
        )
        self._memories.add_resource_link(memory_id, resource.id, relationship=relationship, source=source)
        return resource

    # -- relationships ------------------------------------------------------------------

    def link(
        self, source_memory_id: str, relation_type: str, target_memory_id: str,
        source: Optional[str] = None, confidence: Optional[float] = None,
    ) -> MemoryRelationship:
        if relation_type not in _VALID_RELATION_TYPES:
            raise ValidationError(f"unknown relation_type: {relation_type!r}")
        self.get(source_memory_id)
        self.get(target_memory_id)
        try:
            relationship = self._memories.create_relationship(
                source_memory_id, target_memory_id, relation_type, source=source, confidence=confidence,
            )
        except ValueError as exc:
            raise InvalidRelationshipError(str(exc)) from exc
        self._memories.add_event(
            source_memory_id, MemoryEventType.LINKED.value,
            payload={"target": target_memory_id, "relation_type": relation_type},
        )
        return relationship

    def unlink(self, relationship_id: str) -> bool:
        return self._memories.delete_relationship(relationship_id)

    def neighbors(
        self, memory_id: str, relation_type: Optional[str] = None, depth: int = 1, max_depth: int = 2,
        max_nodes: int = 50,
    ) -> list[MemoryRelationship]:
        """Bounded graph traversal (spec Part 4 §40-41, §121): depth and node
        count are always capped, never unlimited-recursive."""
        depth = min(max(depth, 1), max_depth)
        seen_memory_ids = {memory_id}
        frontier = [memory_id]
        collected: list[MemoryRelationship] = []
        for _ in range(depth):
            next_frontier: list[str] = []
            for mid in frontier:
                for rel in self._memories.list_relationships(mid, relation_type=relation_type):
                    if len(collected) >= max_nodes:
                        return collected
                    collected.append(rel)
                    other = rel.target_memory_id if rel.source_memory_id == mid else rel.source_memory_id
                    if other not in seen_memory_ids:
                        seen_memory_ids.add(other)
                        next_frontier.append(other)
            frontier = next_frontier
            if not frontier:
                break
        return collected

    # -- lifecycle ----------------------------------------------------------------------

    def reinforce(self, memory_id: str, source: Optional[str] = None, reason: Optional[str] = None) -> Memory:
        self.get(memory_id)
        memory = self._memories.reinforce(memory_id)
        self._memories.add_event(
            memory_id, MemoryEventType.REINFORCED.value, payload={"source": source, "reason": reason}
        )
        return memory  # type: ignore[return-value]

    def supersede(self, old_memory_id: str, new_memory_id: str) -> None:
        if old_memory_id == new_memory_id:
            raise InvalidRelationshipError("a memory cannot supersede itself")
        self.get(old_memory_id)
        self.get(new_memory_id)
        self._memories.create_relationship(new_memory_id, old_memory_id, "supersedes")
        self._memories.set_status(old_memory_id, MemoryStatus.SUPERSEDED.value)
        self._memories.add_event(
            old_memory_id, MemoryEventType.SUPERSEDED.value, payload={"by": new_memory_id}
        )

    def correct(self, old_memory_id: str, new_memory_id: str) -> None:
        if old_memory_id == new_memory_id:
            raise InvalidRelationshipError("a memory cannot correct itself")
        self.get(old_memory_id)
        self.get(new_memory_id)
        self._memories.create_relationship(new_memory_id, old_memory_id, "corrects")
        self._memories.set_status(old_memory_id, MemoryStatus.SUPERSEDED.value)
        self._memories.add_event(
            old_memory_id, MemoryEventType.CORRECTED.value, payload={"by": new_memory_id}
        )

    def archive(self, memory_id: str) -> Memory:
        self.get(memory_id)
        memory = self._memories.set_status(memory_id, MemoryStatus.ARCHIVED.value)
        self._memories.add_event(memory_id, MemoryEventType.ARCHIVED.value)
        return memory  # type: ignore[return-value]

    def forget(self, memory_id: str, reason: Optional[str] = None, workspace_id: Optional[str] = None) -> Memory:
        """Logical forgetting only (spec Part 4 §65-67, Part 7 §21-22): status
        flips, provenance/history stay. Hard deletion is a separate, explicit
        operation — see `hard_delete`. [workspace_id], when given, is
        enforced the same fail-closed way as `get()` -- this is about to
        become reachable from cortex_api's "delete conversation" flow, so it
        must never let one tenant forget another tenant's memory."""
        self.get(memory_id, workspace_id=workspace_id)
        memory = self._memories.set_status(memory_id, MemoryStatus.FORGOTTEN.value)
        self._memories.add_event(memory_id, MemoryEventType.FORGOTTEN.value, payload={"reason": reason})
        return memory  # type: ignore[return-value]

    def restore(self, memory_id: str) -> Memory:
        memory = self._memories.get(memory_id, include_deleted=True)
        if memory is None:
            raise MemoryNotFoundError(f"memory {memory_id!r} not found")
        if memory.status == MemoryStatus.DELETED.value:
            raise InvalidMemoryStatusError("hard-deleted memories cannot be restored")
        restored = self._memories.set_status(memory_id, MemoryStatus.ACTIVE.value)
        self._memories.add_event(memory_id, MemoryEventType.RESTORED.value)
        return restored  # type: ignore[return-value]

    def hard_delete(self, memory_id: str) -> None:
        """Explicit, destructive (spec Part 7 §23-24, §58). Removes the
        memory's own document/embedding but never touches external Resources
        (spec Part 7 §26) or the memory_event audit trail (spec Part 7 §31)."""
        self.get(memory_id, touch=False)
        self._documents.delete(memory_id)
        self._memories.set_status(memory_id, MemoryStatus.DELETED.value)
        self._memories.add_event(memory_id, MemoryEventType.DELETED.value)
