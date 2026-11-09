from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.dal.repositories.tag_repository import TagRepository
from lib.presentation.api.deps import get_tag_repo
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.tag import TagOut

router = APIRouter(prefix="/api/v1/tags", tags=["tags"])


@router.get("/search")
def search_tags(q: str = "", limit: int = 20, repo: TagRepository = Depends(get_tag_repo)) -> DataResponse[list[TagOut]]:
    tags = repo.search(q, limit=limit) if q else repo.list_all(limit=limit)
    return DataResponse(data=[TagOut.model_validate(t) for t in tags])
