from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from pydantic import BaseModel, ConfigDict, Field


class MemoryOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    memory_type: str
    status: str
    title: Optional[str] = None
    summary: Optional[str] = None
    content: Optional[str] = None
    importance: Optional[float] = None
    confidence: Optional[float] = None
    created_at: datetime
    updated_at: datetime
    observed_at: Optional[datetime] = None
    valid_from: Optional[datetime] = None
    valid_until: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    last_accessed_at: Optional[datetime] = None
    access_count: int = 0
    reinforcement_count: int = 0


class MemoryCreateRequest(BaseModel):
    content: Optional[str] = None
    memory_type: Optional[str] = None
    title: Optional[str] = None
    summary: Optional[str] = None
    importance: Optional[float] = None
    confidence: Optional[float] = None
    tags: list[str] = Field(default_factory=list)
    entities: list[dict[str, Any]] = Field(default_factory=list)
    resources: list[dict[str, Any]] = Field(default_factory=list)
    context: dict[str, Any] = Field(default_factory=dict)
    provenance: dict[str, Any] = Field(default_factory=dict)
    observed_at: Optional[datetime] = None
    valid_from: Optional[datetime] = None
    valid_until: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class MemoryUpdateRequest(BaseModel):
    title: Optional[str] = None
    summary: Optional[str] = None
    importance: Optional[float] = None
    confidence: Optional[float] = None
    valid_from: Optional[datetime] = None
    valid_until: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    metadata: Optional[dict[str, Any]] = None


class TagRequest(BaseModel):
    tags: list[str]
    source: str = "user"


class EntityAssociateRequest(BaseModel):
    entity_type: str
    canonical_name: str
    role: Optional[str] = None
    source: Optional[str] = None
    confidence: Optional[float] = None


class ResourceLinkRequest(BaseModel):
    resource_type: str
    source_system: str
    external_id: Optional[str] = None
    uri: Optional[str] = None
    relationship: str = "references"


class RelationshipCreateRequest(BaseModel):
    target_memory_id: str
    relation_type: str
    confidence: Optional[float] = None
    source: Optional[str] = None


class RelationshipOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    source_memory_id: str
    target_memory_id: str
    relation_type: str
    confidence: Optional[float] = None
    source: Optional[str] = None
    created_at: datetime


class ReinforceRequest(BaseModel):
    source: Optional[str] = None
    reason: Optional[str] = None


class SupersedeRequest(BaseModel):
    new_memory_id: str


class CorrectRequest(BaseModel):
    new_memory_id: str


class ForgetRequest(BaseModel):
    reason: Optional[str] = None


class ProvenanceOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    source_type: str
    source_system: Optional[str] = None
    source_resource_id: Optional[str] = None
    source_memory_id: Optional[str] = None
    actor_type: Optional[str] = None
    actor_id: Optional[str] = None
    confidence: Optional[float] = None
    created_at: datetime


class EventOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    event_type: str
    actor_type: Optional[str] = None
    actor_id: Optional[str] = None
    payload: dict[str, Any] = Field(default_factory=dict)
    created_at: datetime
