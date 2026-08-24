from __future__ import annotations

from typing import Any, Optional

from pydantic import BaseModel, Field

from lib.presentation.api.schemas.memory import MemoryOut


class RecallRequestIn(BaseModel):
    query: str
    workspace_id: Optional[str] = None
    context: dict[str, Any] = Field(default_factory=dict)
    tags: list[str] = Field(default_factory=list)
    entity_ids: list[str] = Field(default_factory=list)
    resource_ids: list[str] = Field(default_factory=list)
    memory_types: list[str] = Field(default_factory=list)
    historical: bool = False
    limit: int = 10


class RecallResultItem(BaseModel):
    memory: MemoryOut
    score: float
    signals: dict[str, float]
    match_reasons: list[str]
