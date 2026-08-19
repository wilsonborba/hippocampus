from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.dal.remote.working_memory_store import WorkingMemoryStore
from lib.presentation.api.deps import get_working_memory_store
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.working import WorkingMemoryWrite

router = APIRouter(prefix="/api/v1/working", tags=["working"])


@router.put("/{session_id}")
def write_working_memory(
    session_id: str, body: WorkingMemoryWrite, store: WorkingMemoryStore = Depends(get_working_memory_store)
) -> DataResponse[dict]:
    """Transient session context (spec Part 5 §33-34) — never a durable
    Memory by itself; promotion is a separate, deliberate step."""
    store.put(session_id, body.model_dump(), ttl_seconds=body.ttl_seconds)
    return DataResponse(data={"session_id": session_id})


@router.get("/{session_id}")
def get_working_memory(
    session_id: str, store: WorkingMemoryStore = Depends(get_working_memory_store)
) -> DataResponse[dict | None]:
    return DataResponse(data=store.get(session_id))


@router.delete("/{session_id}", status_code=204)
def delete_working_memory(
    session_id: str, store: WorkingMemoryStore = Depends(get_working_memory_store)
) -> None:
    store.delete(session_id)
