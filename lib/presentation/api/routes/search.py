from __future__ import annotations

from fastapi import APIRouter, Depends

from lib.dal.repositories.memory_repository import MemoryRepository
from lib.domain.models import SearchFilters
from lib.domain.services.embedding import EmbeddingProvider, cosine_similarity
from lib.domain.services.memory_service import normalize_workspace_id
from lib.domain.services.memory_service import MemoryService
from lib.presentation.api.deps import get_embedding_provider, get_memory_repo, get_memory_service
from lib.presentation.api.schemas.common import DataResponse
from lib.presentation.api.schemas.memory import MemoryOut
from lib.presentation.api.schemas.search import SearchRequest, SemanticSearchRequest

router = APIRouter(prefix="/api/v1/search", tags=["search"])


@router.post("")
def search(body: SearchRequest, service: MemoryService = Depends(get_memory_service)) -> DataResponse[list[MemoryOut]]:
    """Deterministic structured search (spec Part 5 §24-26) — no embeddings
    involved, so it works identically with or without an embedding provider."""
    filters = SearchFilters(
        text=body.text, workspace_id=body.workspace_id, memory_types=body.memory_types, statuses=body.statuses,
        tags_any=body.tags, tags_all=body.tags_all, entity_id=body.entity_id, resource_id=body.resource_id,
        created_after=body.created_after, created_before=body.created_before, limit=body.limit,
    )
    memories = service.search(filters)
    return DataResponse(data=[MemoryOut.model_validate(m) for m in memories])


@router.post("/semantic")
def semantic_search(
    body: SemanticSearchRequest,
    repo: MemoryRepository = Depends(get_memory_repo),
    embeddings: EmbeddingProvider = Depends(get_embedding_provider),
) -> DataResponse[list[MemoryOut]]:
    """Vector-nearest candidates only (spec Part 5 §27) — this is NOT recall;
    it returns an empty (not erroring) result set when no embedding provider
    is configured, per the graceful-AI-degradation invariant."""
    query_vector = embeddings.embed(body.query)
    if not query_vector:
        return DataResponse(data=[])
    scored = []
    for row in repo.list_embeddings(model=embeddings.name):
        similarity = cosine_similarity(query_vector, row.embedding)
        scored.append((similarity, row.memory_id))
    scored.sort(key=lambda pair: pair[0], reverse=True)
    top_ids = [memory_id for _, memory_id in scored[: body.limit]]
    memories = [repo.get(mid) for mid in top_ids]
    workspace_id = body.filters.get("workspace_id")
    if workspace_id:
        normalized_workspace_id = normalize_workspace_id(workspace_id)
        memories = [m for m in memories if m is not None and m.workspace_id == normalized_workspace_id]
    return DataResponse(data=[MemoryOut.model_validate(m) for m in memories if m is not None])
