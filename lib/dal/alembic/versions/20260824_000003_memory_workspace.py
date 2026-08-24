"""Add workspace scoping to memories

Revision ID: 20260824_000003
Revises: 20260819_000002
Create Date: 2026-08-24 14:55:00.000000
"""
from __future__ import annotations

from alembic import op
import sqlalchemy as sa

revision = "20260824_000003"
down_revision = "20260819_000002"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column(
        "memory",
        sa.Column("workspace_id", sa.String(length=64), nullable=False, server_default="default"),
    )
    op.create_index(op.f("ix_memory_workspace_id"), "memory", ["workspace_id"], unique=False)
    op.create_index(
        "idx_memory_workspace_status_type",
        "memory",
        ["workspace_id", "status", "memory_type"],
        unique=False,
    )
    op.alter_column("memory", "workspace_id", server_default=None)


def downgrade() -> None:
    op.drop_index("idx_memory_workspace_status_type", table_name="memory")
    op.drop_index(op.f("ix_memory_workspace_id"), table_name="memory")
    op.drop_column("memory", "workspace_id")
