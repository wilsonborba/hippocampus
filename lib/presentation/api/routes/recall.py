from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.domain.models import RecallRequest
from lib.domain.services.recall_service import RecallService
from lib.presentation.api.deps import get_recall_service
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.memory import MemoryOut
from lib.presentation.api.schemas.recall import RecallRequestIn, RecallResultItem

router = APIRouter(prefix="/api/v1/recall", tags=["recall"])


@router.post("")
def recall(body: RecallRequestIn, service: RecallService = Depends(get_recall_service)) -> DataResponse[list[RecallResultItem]]:
    request = RecallRequest(
        query=body.query, workspace_id=body.workspace_id, context=body.context, tags=body.tags, entity_ids=body.entity_ids,
        resource_ids=body.resource_ids, memory_types=body.memory_types, historical=body.historical,
        limit=body.limit,
    )
    results = service.recall(request)
    return DataResponse(
        data=[
            RecallResultItem(
                memory=MemoryOut.model_validate(r.memory), score=r.score, signals=r.signals,
                match_reasons=r.match_reasons,
            )
            for r in results
        ]
    )
