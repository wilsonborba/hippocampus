from __future__ import annotations

from typing import Optional

from fastapi import APIRouter, Depends, Query

from lib.dal.repositories.memory_repository import MemoryRepository
from lib.domain.models import MemoryInput
from lib.domain.services.memory_service import MemoryService
from lib.presentation.api.auth import require_admin
from lib.presentation.api.deps import get_memory_repo, get_memory_service
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.memory import (
    CorrectRequest,
    EntityAssociateRequest,
    EventOut,
    ForgetRequest,
    MemoryCreateRequest,
    MemoryOut,
    MemoryUpdateRequest,
    ProvenanceOut,
    ReinforceRequest,
    RelationshipCreateRequest,
    RelationshipOut,
    ResourceLinkRequest,
    SupersedeRequest,
    TagRequest,
)
from lib.presentation.api.schemas.entity import EntityOut
from lib.presentation.api.schemas.resource import ResourceOut
from lib.presentation.api.schemas.tag import TagOut

router = APIRouter(prefix="/api/v1/memories", tags=["memories"])


@router.post("", status_code=201)
def create_memory(
    body: MemoryCreateRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[MemoryOut]:
    memory = service.remember(
        MemoryInput(
            content=body.content, memory_type=body.memory_type, title=body.title,
            summary=body.summary, importance=body.importance, confidence=body.confidence,
            tags=body.tags, entities=body.entities, resources=body.resources,
            context=body.context, provenance=body.provenance, observed_at=body.observed_at,
            valid_from=body.valid_from, valid_until=body.valid_until, expires_at=body.expires_at,
            metadata=body.metadata,
        )
    )
    return DataResponse(data=MemoryOut.model_validate(memory))


@router.get("/{memory_id}")
def get_memory(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[MemoryOut]:
    memory = service.get(memory_id, touch=True)
    return DataResponse(data=MemoryOut.model_validate(memory))


@router.get("")
def list_memories(
    memory_type: Optional[str] = None,
    status: Optional[str] = None,
    tag: Optional[str] = Query(None),
    text: Optional[str] = None,
    limit: int = 20,
    repo: MemoryRepository = Depends(get_memory_repo),
) -> DataResponse[list[MemoryOut]]:
    memories = repo.list(
        memory_type=memory_type, statuses=[status] if status else None,
        tags_any=[tag] if tag else None, text=text, limit=limit,
    )
    return DataResponse(data=[MemoryOut.model_validate(m) for m in memories])


@router.patch("/{memory_id}")
def update_memory(
    memory_id: str, body: MemoryUpdateRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[MemoryOut]:
    fields = {}
    if body.title is not None:
        fields["title"] = body.title
    if body.summary is not None:
        fields["summary"] = body.summary
    if body.importance is not None:
        fields["importance"] = body.importance
    if body.confidence is not None:
        fields["confidence"] = body.confidence
    if body.valid_from is not None:
        fields["valid_from"] = body.valid_from
    if body.valid_until is not None:
        fields["valid_until"] = body.valid_until
    if body.expires_at is not None:
        fields["expires_at"] = body.expires_at
    if body.metadata is not None:
        fields["metadata_json"] = body.metadata
    memory = service.update(memory_id, fields)
    return DataResponse(data=MemoryOut.model_validate(memory))


@router.post("/{memory_id}/tags")
def tag_memory(
    memory_id: str, body: TagRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[list[TagOut]]:
    tags = service.tag(memory_id, body.tags, source=body.source)
    return DataResponse(data=[TagOut.model_validate(t) for t in tags])


@router.delete("/{memory_id}/tags/{tag_id}", status_code=204)
def untag_memory(memory_id: str, tag_id: str, service: MemoryService = Depends(get_memory_service)) -> None:
    service.untag(memory_id, tag_id)


@router.post("/{memory_id}/entities")
def associate_entity(
    memory_id: str, body: EntityAssociateRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[EntityOut]:
    entity = service.associate_entity(
        memory_id, body.entity_type, body.canonical_name, role=body.role,
        source=body.source, confidence=body.confidence,
    )
    return DataResponse(data=EntityOut.model_validate(entity))


@router.post("/{memory_id}/resources")
def associate_resource(
    memory_id: str, body: ResourceLinkRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[ResourceOut]:
    resource = service.associate_resource(
        memory_id, body.resource_type, body.source_system, external_id=body.external_id,
        uri=body.uri, relationship=body.relationship,
    )
    return DataResponse(data=ResourceOut.model_validate(resource))


@router.post("/{memory_id}/relationships", status_code=201)
def create_relationship(
    memory_id: str, body: RelationshipCreateRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[RelationshipOut]:
    relationship = service.link(
        memory_id, body.relation_type, body.target_memory_id, source=body.source, confidence=body.confidence,
    )
    return DataResponse(data=RelationshipOut.model_validate(relationship))


@router.get("/{memory_id}/neighbors")
def get_neighbors(
    memory_id: str, relation_type: Optional[str] = None, depth: int = 1,
    service: MemoryService = Depends(get_memory_service),
) -> DataResponse[list[RelationshipOut]]:
    service.get(memory_id)
    relationships = service.neighbors(memory_id, relation_type=relation_type, depth=depth)
    return DataResponse(data=[RelationshipOut.model_validate(r) for r in relationships])


@router.post("/{memory_id}/reinforce")
def reinforce_memory(
    memory_id: str, body: ReinforceRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[MemoryOut]:
    memory = service.reinforce(memory_id, source=body.source, reason=body.reason)
    return DataResponse(data=MemoryOut.model_validate(memory))


@router.post("/{memory_id}/supersede")
def supersede_memory(
    memory_id: str, body: SupersedeRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[dict]:
    service.supersede(memory_id, body.new_memory_id)
    return DataResponse(data={"memory_id": memory_id, "status": "superseded"})


@router.post("/{memory_id}/correct")
def correct_memory(
    memory_id: str, body: CorrectRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[dict]:
    service.correct(memory_id, body.new_memory_id)
    return DataResponse(data={"memory_id": memory_id, "status": "superseded"})


@router.post("/{memory_id}/archive")
def archive_memory(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[dict]:
    service.archive(memory_id)
    return DataResponse(data={"memory_id": memory_id, "status": "archived"})


@router.post("/{memory_id}/forget")
def forget_memory(
    memory_id: str, body: ForgetRequest, service: MemoryService = Depends(get_memory_service)
) -> DataResponse[dict]:
    service.forget(memory_id, reason=body.reason)
    return DataResponse(data={"memory_id": memory_id, "status": "forgotten"})


@router.post("/{memory_id}/restore")
def restore_memory(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[MemoryOut]:
    memory = service.restore(memory_id)
    return DataResponse(data=MemoryOut.model_validate(memory))


@router.delete("/{memory_id}", dependencies=[Depends(require_admin)])
def hard_delete_memory(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[dict]:
    """Explicit destructive operation (spec Part 5 §58) — never triggered by
    `forget`. Requires an admin-scoped service identity (spec Part 7 §10)."""
    service.hard_delete(memory_id)
    return DataResponse(data={"memory_id": memory_id, "status": "deleted"})


@router.get("/{memory_id}/provenance")
def get_provenance(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[list[ProvenanceOut]]:
    service.get(memory_id)
    rows = service.provenance(memory_id)
    return DataResponse(data=[ProvenanceOut.model_validate(r) for r in rows])


@router.get("/{memory_id}/events")
def get_events(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[list[EventOut]]:
    service.get(memory_id)
    rows = service.events(memory_id)
    return DataResponse(data=[EventOut.model_validate(r) for r in rows])


@router.get("/{memory_id}/document")
def get_document(memory_id: str, service: MemoryService = Depends(get_memory_service)) -> DataResponse[Optional[dict]]:
    service.get(memory_id)
    return DataResponse(data=service.document(memory_id))
