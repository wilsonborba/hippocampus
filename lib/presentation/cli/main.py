from __future__ import annotations

from importlib.metadata import PackageNotFoundError, version
from typing import Optional

import typer

from lib.core.logs import LogTarget, configure_logging, get_logger
from lib.core.settings import get_settings
from lib.domain.models import RecallRequest, SearchFilters
from lib.presentation.api.deps import get_memory_service, get_recall_service
from lib.presentation.cli import memory_cmd
from lib.presentation.cli.output import CliState, console, emit, print_table

logger = get_logger(__name__)

app = typer.Typer(
    name="hippocampus",
    help="Dedicated memory service: remember, recall, search, and manage durable memory.",
    no_args_is_help=True,
)
app.add_typer(memory_cmd.app, name="memory")


def _version() -> str:
    try:
        return version("hippocampus")
    except PackageNotFoundError:
        return "0.1.0"


def _version_callback(show: bool) -> None:
    if show:
        console.print(f"hippocampus {_version()}")
        raise typer.Exit()


@app.callback()
def main(
    ctx: typer.Context,
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show debug-level logs"),
    json_output: bool = typer.Option(False, "--json", help="Force JSON output"),
    version_: bool = typer.Option(
        False, "--version", callback=_version_callback, is_eager=True, help="Show the version and exit"
    ),
) -> None:
    ctx.obj = CliState(verbose=verbose, json_output=json_output)
    settings = get_settings()
    configure_logging(target=LogTarget.CLI, verbose=verbose, log_file=settings.log_file)


# -- search / recall ----------------------------------------------------------------


@app.command()
def search(
    ctx: typer.Context,
    query: str = typer.Argument(...),
    type_: Optional[str] = typer.Option(None, "--type"),
    tag: Optional[str] = typer.Option(None, "--tag"),
    limit: int = typer.Option(20, "--limit"),
) -> None:
    filters = SearchFilters(
        text=query, memory_types=[type_] if type_ else [], tags_any=[tag] if tag else [], limit=limit,
    )
    memories = get_memory_service().search(filters)

    def _render() -> None:
        print_table(
            "Search results", ["ID", "TYPE", "STATUS", "SUMMARY"],
            [[m.id, m.memory_type, m.status, (m.title or m.summary or "")[:60]] for m in memories],
        )

    emit(ctx, json_data=[{"id": m.id, "memory_type": m.memory_type, "status": m.status} for m in memories], table=_render)


@app.command()
def recall(
    ctx: typer.Context,
    query: str = typer.Argument(...),
    tag: Optional[str] = typer.Option(None, "--tag"),
    historical: bool = typer.Option(False, "--historical"),
    limit: int = typer.Option(10, "--limit"),
) -> None:
    request = RecallRequest(query=query, tags=[tag] if tag else [], historical=historical, limit=limit)
    results = get_recall_service().recall(request)

    def _render() -> None:
        for i, r in enumerate(results, start=1):
            summary = r.memory.title or r.memory.summary or r.memory.content or ""
            console.print(f"[bold]{i}. {summary[:80]}[/bold]")
            console.print(f"   score: {r.score}")
            console.print(f"   type: {r.memory.memory_type}   status: {r.memory.status}")
            if r.match_reasons:
                console.print(f"   matched: {', '.join(r.match_reasons)}")
            console.print("")
        if not results:
            console.print("[dim]No results.[/dim]")

    emit(
        ctx,
        json_data=[
            {"id": r.memory.id, "score": r.score, "signals": r.signals, "match_reasons": r.match_reasons}
            for r in results
        ],
        table=_render,
    )


# -- health -------------------------------------------------------------------------


@app.command()
def health(ctx: typer.Context) -> None:
    from lib.presentation.api.routes.health import ready

    result = ready()

    def _render() -> None:
        console.print(f"status: {result['status']}")
        print_table("Dependencies", ["DEPENDENCY", "STATE"], list(result["dependencies"].items()))

    emit(ctx, json_data=result, table=_render)


# -- db ---------------------------------------------------------------------------

db_app = typer.Typer(help="Database maintenance.")
app.add_typer(db_app, name="db")


@db_app.command("upgrade")
def db_upgrade(ctx: typer.Context) -> None:
    from lib.dal.migrations import upgrade_db

    upgrade_db()
    console.print("Database upgraded to head.")


# -- admin --------------------------------------------------------------------------

admin_app = typer.Typer(help="Background maintenance jobs (run manually until a scheduler exists).")
app.add_typer(admin_app, name="admin")


@admin_app.command("reconcile")
def admin_reconcile(
    ctx: typer.Context, dry_run: bool = typer.Option(True, "--dry-run/--apply"),
) -> None:
    from lib.domain.tasks import reconciliation_worker
    from lib.presentation.api.deps import get_document_store, get_memory_repo

    report = reconciliation_worker.run(get_memory_repo(), get_document_store(), dry_run=dry_run)
    emit(
        ctx,
        json_data={"missing_document": report.missing_document, "missing_embedding": report.missing_embedding},
        table=lambda: (
            console.print(f"missing_document: {len(report.missing_document)}"),
            console.print(f"missing_embedding: {len(report.missing_embedding)}"),
        ),
    )


@admin_app.command("reembed")
def admin_reembed(ctx: typer.Context, batch_limit: int = typer.Option(50, "--limit")) -> None:
    from lib.domain.tasks import embedding_worker
    from lib.presentation.api.deps import get_embedding_provider, get_memory_repo

    report = embedding_worker.run(get_memory_repo(), get_embedding_provider(), batch_limit=batch_limit)
    emit(ctx, json_data=report.__dict__, table=lambda: console.print(report))


@admin_app.command("expire")
def admin_expire(ctx: typer.Context) -> None:
    from lib.domain.tasks import expiration_worker
    from lib.presentation.api.deps import get_memory_repo

    report = expiration_worker.run(get_memory_repo())
    emit(ctx, json_data={"expired_memory_ids": report.expired_memory_ids})


@admin_app.command("sync-documents")
def admin_sync_documents(ctx: typer.Context, batch_limit: int = typer.Option(50, "--limit")) -> None:
    from lib.domain.tasks import document_sync_worker
    from lib.presentation.api.deps import get_document_store, get_memory_repo

    report = document_sync_worker.run(get_memory_repo(), get_document_store(), batch_limit=batch_limit)
    emit(ctx, json_data=report.__dict__, table=lambda: console.print(report))
