from __future__ import annotations

from datetime import datetime
from typing import Literal, Optional

from pydantic import BaseModel, Field


class ConsolidationFilters(BaseModel):
    memory_types: list[str] = Field(default_factory=list)
    tags: list[str] = Field(default_factory=list)
    created_after: Optional[datetime] = None
    created_before: Optional[datetime] = None
    limit: int = 50


class ConsolidationRequest(BaseModel):
    memory_ids: list[str] = Field(default_factory=list)
    filters: Optional[ConsolidationFilters] = None
    mode: Literal["analyze", "apply_safe"] = "analyze"


class DuplicateCandidateOut(BaseModel):
    memory_a_id: str
    memory_b_id: str
    reason: str


class ContradictionCandidateOut(BaseModel):
    memory_a_id: str
    memory_b_id: str
    reason: str


class AppliedChangeOut(BaseModel):
    action: str
    memory_id: str
    detail: str


class ConsolidationResultOut(BaseModel):
    candidate_duplicates: list[DuplicateCandidateOut]
    candidate_contradictions: list[ContradictionCandidateOut]
    applied_changes: list[AppliedChangeOut]
