from __future__ import annotations

from datetime import datetime
from typing import Optional

from pydantic import BaseModel, ConfigDict


class ResourceOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    resource_type: str
    source_system: str
    external_id: Optional[str] = None
    uri: Optional[str] = None
    title: Optional[str] = None
    ownership: str = "external"
    status: str
    content_type: Optional[str] = None
    checksum: Optional[str] = None
    size_bytes: Optional[int] = None
    created_at: datetime


class ResourceRegisterRequest(BaseModel):
    resource_type: str
    source_system: str
    external_id: Optional[str] = None
    uri: Optional[str] = None
    title: Optional[str] = None
    content_type: Optional[str] = None
    checksum: Optional[str] = None
    size_bytes: Optional[int] = None
