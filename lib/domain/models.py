from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Optional

from lib.dal.models import Memory, MemoryProvenance, MemoryRelationship, Resource, Tag


@dataclass
class MemoryInput:
    """Bundles a `remember` call's optional fields (spec Part 5 §17). Explicit
    values here always win over anything an enrichment step might infer later
    (spec Part 4 §6 - "explicit metadata has precedence")."""

    content: Optional[str] = None
    memory_type: Optional[str] = None
    title: Optional[str] = None
    summary: Optional[str] = None
    importance: Optional[float] = None
    confidence: Optional[float] = None
    tags: list[str] = field(default_factory=list)
    entities: list[dict[str, Any]] = field(default_factory=list)  # {entity_type, canonical_name, role?}
    resources: list[dict[str, Any]] = field(default_factory=list)  # {resource_type, source_system, ...}
    context: dict[str, Any] = field(default_factory=dict)
    provenance: dict[str, Any] = field(default_factory=dict)
    observed_at: Optional[datetime] = None
    valid_from: Optional[datetime] = None
    valid_until: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    metadata: dict[str, Any] = field(default_factory=dict)
    created_by: Optional[str] = None
    document: Optional[dict[str, Any]] = None  # rich content bound for the MemoryDocument store


@dataclass
class SearchFilters:
    """Deterministic structured-search criteria (spec Part 4 §14-16)."""

    text: Optional[str] = None
    memory_types: list[str] = field(default_factory=list)
    statuses: list[str] = field(default_factory=list)
    tags_any: list[str] = field(default_factory=list)
    tags_all: list[str] = field(default_factory=list)
    entity_id: Optional[str] = None
    resource_id: Optional[str] = None
    created_after: Optional[datetime] = None
    created_before: Optional[datetime] = None
    observed_after: Optional[datetime] = None
    observed_before: Optional[datetime] = None
    limit: int = 20
    cursor: Optional[str] = None


@dataclass
class RecallRequest:
    """Recall input (spec Part 5 §29). Only `query` is required; everything
    else narrows/reweights candidates rather than gating them out entirely,
    except where noted (tags/entities act as hard filters here for the MVP
    weighted model, matching the "eligibility filtering" step in Part 4 §26)."""

    query: str
    context: dict[str, Any] = field(default_factory=dict)
    tags: list[str] = field(default_factory=list)
    entity_ids: list[str] = field(default_factory=list)
    resource_ids: list[str] = field(default_factory=list)
    memory_types: list[str] = field(default_factory=list)
    historical: bool = False
    limit: int = 10


@dataclass
class RecallResult:
    """Structured recall evidence (spec Part 4 §21, Part 5 §30-31) — never an
    opaque generated paragraph."""

    memory: Memory
    score: float
    signals: dict[str, float]
    match_reasons: list[str]
    tags: list[Tag] = field(default_factory=list)
    relationships: list[MemoryRelationship] = field(default_factory=list)
    provenance: list[MemoryProvenance] = field(default_factory=list)
    resources: list[Resource] = field(default_factory=list)
