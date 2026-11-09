from __future__ import annotations

from typing import Any, Generic, Optional, TypeVar

from pydantic import BaseModel

T = TypeVar("T")


class Pagination(BaseModel):
    next_cursor: Optional[str] = None


class DataResponse(BaseModel, Generic[T]):
    """Consistent response envelope (spec Part 5 §6)."""

    data: T
    pagination: Optional[Pagination] = None


class ErrorBody(BaseModel):
    code: str
    message: str
    details: dict[str, Any] = {}


class ErrorResponse(BaseModel):
    """Machine-readable error envelope (spec Part 5 §7-8)."""

    error: ErrorBody
