"""Convert memory_embedding.embedding to a native pgvector column on PostgreSQL

Revision ID: 20260819_000002
Revises: 20260819_000001
Create Date: 2026-08-19 18:00:00.000000
"""
from __future__ import annotations

from alembic import op

revision = "20260819_000002"
down_revision = "20260819_000001"
branch_labels = None
depends_on = None


def upgrade() -> None:
    bind = op.get_bind()
    if bind.dialect.name != "postgresql":
        # SQLite (dev/test) keeps the JSON column: `PortableVector` resolves
        # to JSON there and tests build the schema fresh via
        # `Base.metadata.create_all`, never through this migration path.
        return
    op.execute("CREATE EXTENSION IF NOT EXISTS vector")
    # No fixed dimension yet (spec Part 7 §147 — model still not chosen): a
    # dimension-less `vector` column stores any length, at the cost of not
    # being ANN-indexable until a follow-up migration pins one.
    op.execute(
        "ALTER TABLE memory_embedding "
        "ALTER COLUMN embedding TYPE vector USING embedding::text::vector"
    )


def downgrade() -> None:
    bind = op.get_bind()
    if bind.dialect.name != "postgresql":
        return
    op.execute(
        "ALTER TABLE memory_embedding "
        "ALTER COLUMN embedding TYPE json USING to_json(embedding::text)"
    )
