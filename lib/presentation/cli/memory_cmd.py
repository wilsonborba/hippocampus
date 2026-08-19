from __future__ import annotations

from pathlib import Path
from typing import List, Optional

import typer

from lib.domain.errors import DomainError
from lib.domain.models import MemoryInput
from lib.presentation.api.deps import get_memory_service
from lib.presentation.cli.output import emit, error_exit, print_table

app = typer.Typer(help="Create, inspect, and manage memories.", no_args_is_help=True)


def _memory_row(m) -> list:
    return [m.id, m.memory_type, m.status, (m.title or m.summary or m.content or "")[:60]]


@app.command()
def remember(
    ctx: typer.Context,
    content: Optional[str] = typer.Option(None, "--content", help="Memory content"),
    content_file: Optional[Path] = typer.Option(None, "--content-file", help="Read content from a file"),
    stdin: bool = typer.Option(False, "--stdin", help="Read content from stdin"),
    type_: Optional[str] = typer.Option(None, "--type", help="working|episodic|semantic|decision|..."),
    title: Optional[str] = typer.Option(None, "--title"),
    summary: Optional[str] = typer.Option(None, "--summary"),
    importance: Optional[float] = typer.Option(None, "--importance"),
    confidence: Optional[float] = typer.Option(None, "--confidence"),
    tag: List[str] = typer.Option([], "--tag", help="namespace:value, repeatable"),
) -> None:
    if stdin:
        import sys

        content = sys.stdin.read()
    elif content_file:
        content = content_file.read_text()

    try:
        memory = get_memory_service().remember(
            MemoryInput(
                content=content, memory_type=type_, title=title, summary=summary,
                importance=importance, confidence=confidence, tags=list(tag),
            )
        )
    except DomainError as exc:
        error_exit(str(exc))
        return

    def _render() -> None:
        print_table("Memory created", ["ID", "TYPE", "STATUS", "SUMMARY"], [_memory_row(memory)])

    emit(ctx, json_data={"id": memory.id, "memory_type": memory.memory_type, "status": memory.status}, table=_render)


@app.command()
def get(ctx: typer.Context, memory_id: str) -> None:
    try:
        memory = get_memory_service().get(memory_id, touch=True)
    except DomainError as exc:
        error_exit(str(exc))
        return

    def _render() -> None:
        print_table("Memory", ["ID", "TYPE", "STATUS", "SUMMARY"], [_memory_row(memory)])

    emit(
        ctx,
        json_data={
            "id": memory.id, "memory_type": memory.memory_type, "status": memory.status,
            "title": memory.title, "summary": memory.summary, "content": memory.content,
            "importance": memory.importance, "confidence": memory.confidence,
        },
        table=_render,
    )


@app.command("list")
def list_(
    ctx: typer.Context,
    type_: Optional[str] = typer.Option(None, "--type"),
    status: Optional[str] = typer.Option(None, "--status"),
    tag: Optional[str] = typer.Option(None, "--tag"),
    limit: int = typer.Option(20, "--limit"),
) -> None:
    from lib.presentation.api.deps import get_memory_repo

    memories = get_memory_repo().list(
        memory_type=type_, statuses=[status] if status else None, tags_any=[tag] if tag else None,
        limit=limit,
    )

    def _render() -> None:
        print_table("Memories", ["ID", "TYPE", "STATUS", "SUMMARY"], [_memory_row(m) for m in memories])

    emit(ctx, json_data=[{"id": m.id, "memory_type": m.memory_type, "status": m.status} for m in memories], table=_render)


@app.command()
def tag(ctx: typer.Context, memory_id: str, tags: List[str] = typer.Argument(...)) -> None:
    try:
        added = get_memory_service().tag(memory_id, tags)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data=[t.canonical_name for t in added], table=lambda: print_table(
        "Tagged", ["TAG"], [[t.canonical_name] for t in added]
    ))


@app.command()
def untag(ctx: typer.Context, memory_id: str, tag_id: str) -> None:
    removed = get_memory_service().untag(memory_id, tag_id)
    emit(ctx, json_data={"removed": removed})


@app.command()
def link(
    ctx: typer.Context, source_memory_id: str, relation: str, target_memory_id: str,
) -> None:
    """`hippocampus memory link M2 supersedes M1` (spec Part 5 §88)."""
    try:
        relationship = get_memory_service().link(source_memory_id, relation, target_memory_id)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"id": relationship.id, "relation_type": relationship.relation_type})


@app.command()
def reinforce(ctx: typer.Context, memory_id: str, reason: Optional[str] = typer.Option(None, "--reason")) -> None:
    try:
        memory = get_memory_service().reinforce(memory_id, reason=reason)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"id": memory.id, "reinforcement_count": memory.reinforcement_count})


@app.command()
def supersede(ctx: typer.Context, old_memory_id: str, with_: str = typer.Option(..., "--with")) -> None:
    try:
        get_memory_service().supersede(old_memory_id, with_)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"memory_id": old_memory_id, "status": "superseded"})


@app.command()
def correct(ctx: typer.Context, old_memory_id: str, with_: str = typer.Option(..., "--with")) -> None:
    try:
        get_memory_service().correct(old_memory_id, with_)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"memory_id": old_memory_id, "status": "superseded"})


@app.command()
def archive(ctx: typer.Context, memory_id: str) -> None:
    try:
        get_memory_service().archive(memory_id)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"memory_id": memory_id, "status": "archived"})


@app.command()
def forget(ctx: typer.Context, memory_id: str) -> None:
    try:
        get_memory_service().forget(memory_id)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"memory_id": memory_id, "status": "forgotten"})


@app.command()
def delete(
    ctx: typer.Context, memory_id: str,
    hard: bool = typer.Option(False, "--hard", help="Destructive hard delete, not the default `forget`"),
    yes: bool = typer.Option(False, "--yes", help="Skip confirmation (required for non-interactive use)"),
) -> None:
    if not hard:
        error_exit("use `hippocampus memory forget` for logical forgetting, or pass --hard to delete permanently")
        return
    if not yes:
        typer.confirm(f"Permanently delete memory {memory_id}?", abort=True)
    try:
        get_memory_service().hard_delete(memory_id)
    except DomainError as exc:
        error_exit(str(exc))
        return
    emit(ctx, json_data={"memory_id": memory_id, "status": "deleted"})


@app.command()
def provenance(ctx: typer.Context, memory_id: str) -> None:
    try:
        rows = get_memory_service().provenance(memory_id)
    except DomainError as exc:
        error_exit(str(exc))
        return

    def _render() -> None:
        print_table(
            "Provenance", ["SOURCE TYPE", "ACTOR", "CONFIDENCE"],
            [[r.source_type, r.actor_type, r.confidence] for r in rows],
        )

    emit(ctx, json_data=[{"source_type": r.source_type, "actor_type": r.actor_type} for r in rows], table=_render)
