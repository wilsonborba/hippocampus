from __future__ import annotations

from typing import Any, Optional

from pydantic import BaseModel, Field


class WorkingMemoryWrite(BaseModel):
    current_topic: Optional[str] = None
    active_tags: list[str] = Field(default_factory=list)
    active_entities: list[str] = Field(default_factory=list)
    recent_memory_ids: list[str] = Field(default_factory=list)
    temporary_notes: dict[str, Any] = Field(default_factory=dict)
    ttl_seconds: Optional[int] = 3600
