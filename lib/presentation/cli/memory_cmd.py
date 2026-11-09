from __future__ import annotations

from pathlib import Path
from typing import List, Optional

import typer

from lib.domain.errors import DomainError, UnsupportedRenderFormatError
from lib.domain.models import MemoryInput
from lib.domain.rendering import compute_layout, render_d2, render_mermaid, render_png, render_svg
from lib.presentation.api.deps import get_memory_graph_service, get_memory_service
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
    workspace: str = typer.Option("default", "--workspace", help="Workspace/tenant id"),
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
                content=content, workspace_id=workspace, memory_type=type_, title=title, summary=summary,
                importance=importance, confidence=confidence, tags=list(tag),
            )
        )
    except DomainError as exc:
        error_exit(str(exc))
        return

    def _render() -> None:
        print_table("Memory created", ["ID", "TYPE", "STATUS", "SUMMARY"], [_memory_row(memory)])

    emit(
        ctx,
        json_data={
            "id": memory.id, "workspace_id": memory.workspace_id,
            "memory_type": memory.memory_type, "status": memory.status,
        },
        table=_render,
    )


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
            "workspace_id": memory.workspace_id, "title": memory.title, "summary": memory.summary, "content": memory.content,
            "importance": memory.importance, "confidence": memory.confidence,
        },
        table=_render,
    )


@app.command("list")
def list_(
    ctx: typer.Context,
    type_: Optional[str] = typer.Option(None, "--type"),
    workspace: Optional[str] = typer.Option(None, "--workspace"),
    status: Optional[str] = typer.Option(None, "--status"),
    tag: Optional[str] = typer.Option(None, "--tag"),
    limit: int = typer.Option(20, "--limit"),
) -> None:
    from lib.domain.models import SearchFilters
    from lib.presentation.api.deps import get_memory_service

    memories = get_memory_service().search(
        SearchFilters(
            workspace_id=workspace, memory_types=[type_] if type_ else [],
            statuses=[status] if status else [], tags_any=[tag] if tag else [], limit=limit,
        )
    )

    def _render() -> None:
        print_table("Memories", ["ID", "TYPE", "STATUS", "SUMMARY"], [_memory_row(m) for m in memories])

    emit(
        ctx,
        json_data=[
            {"id": m.id, "workspace_id": m.workspace_id, "memory_type": m.memory_type, "status": m.status}
            for m in memories
        ],
        table=_render,
    )


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
def graph(
    ctx: typer.Context,
    memory_id: str,
    depth: Optional[int] = typer.Option(None, "--depth"),
    relation: Optional[str] = typer.Option(None, "--relation", help="comma-separated relation_type filter"),
    include_entities: bool = typer.Option(True, "--include-entities/--no-entities"),
    include_tags: bool = typer.Option(True, "--include-tags/--no-tags"),
    include_resources: bool = typer.Option(True, "--include-resources/--no-resources"),
    max_nodes: int = typer.Option(300, "--max-nodes"),
    format_: str = typer.Option("json", "--format", help="json|d2|mermaid|svg|png"),
    output: Optional[Path] = typer.Option(None, "--output", help="required for svg/png"),
) -> None:
    """`hippocampus memory graph <id> --format svg --output out.svg`."""
    try:
        get_memory_service().get(memory_id)
        graph_data = get_memory_graph_service().build_graph(
            [memory_id], depth=depth,
            relation_types=relation.split(",") if relation else None,
            include_entities=include_entities, include_tags=include_tags,
            include_resources=include_resources, max_nodes=max_nodes,
        )
    except DomainError as exc:
        error_exit(str(exc))
        return

    if format_ in ("svg", "png") and output is None:
        error_exit(f"--format {format_} requires --output <path>")
        return

    if format_ == "json":
        payload = {
            "nodes": [vars(n) for n in graph_data.nodes],
            "edges": [vars(e) for e in graph_data.edges],
            "root_ids": graph_data.root_ids,
            "truncated": graph_data.truncated,
        }
        emit(ctx, json_data=payload)
        return
    if format_ == "d2":
        typer.echo(render_d2(graph_data))
        return
    if format_ == "mermaid":
        typer.echo(render_mermaid(graph_data))
        return
    if format_ == "svg":
        output.write_text(render_svg(compute_layout(graph_data), graph_data))
        typer.echo(f"wrote {output}")
        return
    if format_ == "png":
        output.write_bytes(render_png(render_svg(compute_layout(graph_data), graph_data)))
        typer.echo(f"wrote {output}")
        return

    error_exit(str(UnsupportedRenderFormatError(f"unsupported format: {format_!r}")))


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
