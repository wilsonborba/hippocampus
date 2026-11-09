from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.dal.repositories.resource_repository import ResourceRepository
from lib.domain.errors import ResourceNotFoundError
from lib.presentation.api.deps import get_resource_repo
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.resource import ResourceOut, ResourceRegisterRequest

router = APIRouter(prefix="/api/v1/resources", tags=["resources"])


@router.post("", status_code=201)
def register_resource(
    body: ResourceRegisterRequest, repo: ResourceRepository = Depends(get_resource_repo)
) -> DataResponse[ResourceOut]:
    """Registration only — Hippocampus never requires or stores file bytes
    here (spec Part 5 §43-44)."""
    resource = repo.register(
        resource_type=body.resource_type, source_system=body.source_system,
        external_id=body.external_id, uri=body.uri, title=body.title,
        content_type=body.content_type, checksum=body.checksum, size_bytes=body.size_bytes,
    )
    return DataResponse(data=ResourceOut.model_validate(resource))


@router.get("/{resource_id}")
def get_resource(resource_id: str, repo: ResourceRepository = Depends(get_resource_repo)) -> DataResponse[ResourceOut]:
    resource = repo.get_by_id(resource_id)
    if resource is None:
        raise ResourceNotFoundError(f"resource {resource_id!r} not found")
    return DataResponse(data=ResourceOut.model_validate(resource))
