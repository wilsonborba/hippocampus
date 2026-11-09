from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.dal.repositories.entity_repository import EntityRepository
from lib.presentation.api.deps import get_entity_repo
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.entity import EntityOut

router = APIRouter(prefix="/api/v1/entities", tags=["entities"])


@router.get("/search")
def search_entities(q: str = "", limit: int = 20, repo: EntityRepository = Depends(get_entity_repo)) -> DataResponse[list[EntityOut]]:
    entities = repo.search(q, limit=limit)
    return DataResponse(data=[EntityOut.model_validate(e) for e in entities])


@router.get("/{entity_id}")
def get_entity(entity_id: str, repo: EntityRepository = Depends(get_entity_repo)) -> DataResponse[EntityOut]:
    from lib.domain.errors import EntityNotFoundError

    entity = repo.get_by_id(entity_id)
    if entity is None:
        raise EntityNotFoundError(f"entity {entity_id!r} not found")
    return DataResponse(data=EntityOut.model_validate(entity))
