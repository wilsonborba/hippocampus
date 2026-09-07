from __future__ import annotations

from datetime import datetime

from pydantic import BaseModel, ConfigDict


class TagOut(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: str
    namespace: str
    value: str
    canonical_name: str
    created_at: datetime
