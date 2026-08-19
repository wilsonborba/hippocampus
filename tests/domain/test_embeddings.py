from __future__ import annotations

from lib.domain.models import MemoryInput
from lib.domain.services.embedding import cosine_similarity


def test_embedding_round_trips_through_portable_vector_column(memory_service, memory_repo):
    """On SQLite, `PortableVector` (spec: native pgvector on PostgreSQL,
    portable JSON elsewhere) resolves to JSON — this exercises that fallback
    path end to end. The PostgreSQL path is verified against the real
    database as part of issue #3, not here (no live Postgres in CI)."""
    memory = memory_service.remember(MemoryInput(content="x"))
    memory_repo.upsert_embedding(
        memory.id, model="test-model", model_version="v1", dimensions=3, embedding=[0.1, 0.2, 0.3]
    )

    rows = memory_repo.list_embeddings(model="test-model")
    assert len(rows) == 1
    assert rows[0].memory_id == memory.id
    assert rows[0].embedding == [0.1, 0.2, 0.3]
    assert cosine_similarity(rows[0].embedding, [0.1, 0.2, 0.3]) > 0.999


def test_upsert_embedding_updates_existing_row_for_same_model(memory_service, memory_repo):
    memory = memory_service.remember(MemoryInput(content="x"))
    memory_repo.upsert_embedding(memory.id, model="m", model_version=None, dimensions=2, embedding=[1.0, 0.0])
    memory_repo.upsert_embedding(memory.id, model="m", model_version=None, dimensions=2, embedding=[0.0, 1.0])

    rows = memory_repo.list_embeddings(model="m")
    assert len(rows) == 1
    assert rows[0].embedding == [0.0, 1.0]
