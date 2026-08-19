from __future__ import annotations

from datetime import datetime
from typing import Optional

from pydantic import BaseModel, Field

from lib.presentation.api.schemas.memory import MemoryOut


class SearchRequest(BaseModel):
    text: Optional[str] = None
    memory_types: list[str] = Field(default_factory=list)
    statuses: list[str] = Field(default_factory=list)
    tags: list[str] = Field(default_factory=list)
    entity_id: Optional[str] = None
    resource_id: Optional[str] = None
    created_after: Optional[datetime] = None
    created_before: Optional[datetime] = None
    limit: int = 20
    cursor: Optional[str] = None


class SearchResultItem(BaseModel):
    memory: MemoryOut


class SemanticSearchRequest(BaseModel):
    query: str
    filters: dict = Field(default_factory=dict)
    limit: int = 20
