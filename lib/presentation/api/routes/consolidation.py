from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.domain.models import SearchFilters
from lib.domain.services.consolidation_service import ConsolidationService
from lib.presentation.api.deps import get_consolidation_service
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.consolidation import ConsolidationRequest, ConsolidationResultOut

router = APIRouter(prefix="/api/v1/consolidation", tags=["consolidation"])


@router.post("")
def consolidate(
    body: ConsolidationRequest, service: ConsolidationService = Depends(get_consolidation_service)
) -> DataResponse[ConsolidationResultOut]:
    """`analyze` only ever reports candidates; `apply_safe` additionally
    archives confirmed exact duplicates (spec Part 5 §65-66). Candidate sets
    are always bounded — memory_ids or filters is required, never "everything"."""
    filters = None
    if body.filters:
        filters = SearchFilters(
            workspace_id=body.filters.workspace_id,
            memory_types=body.filters.memory_types, tags_any=body.filters.tags,
            created_after=body.filters.created_after, created_before=body.filters.created_before,
            limit=body.filters.limit,
        )
    method = service.apply_safe if body.mode == "apply_safe" else service.analyze
    report = method(memory_ids=body.memory_ids or None, filters=filters)
    return DataResponse(data=ConsolidationResultOut.model_validate(report, from_attributes=True))
