from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Callable, Optional

from lib.core.logs import get_logger
from lib.core.settings import Settings, get_settings

logger = get_logger(__name__)


@dataclass
class ScheduledJob:
    """One periodic worker invocation (spec Part 6 §103-105, §145). `run` is
    a zero-argument synchronous callable — the already-bound `worker.run(...)`
    functions from lib.domain.tasks — so the scheduler itself never contains
    job-specific logic, only timing."""

    name: str
    interval_seconds: float
    run: Callable[[], None]


async def _job_loop(job: ScheduledJob, stop_event: asyncio.Event) -> None:
    while not stop_event.is_set():
        try:
            await asyncio.to_thread(job.run)
        except Exception:
            # A failed job must not crash the scheduler or the other jobs
            # (spec Part 6 Invariant 2 — optional background work failing
            # never invalidates anything else).
            logger.warning("scheduled job %r failed", job.name, exc_info=True)
        try:
            await asyncio.wait_for(stop_event.wait(), timeout=job.interval_seconds)
        except asyncio.TimeoutError:
            pass  # normal case: interval elapsed, loop again


async def run_scheduler(jobs: list[ScheduledJob], stop_event: Optional[asyncio.Event] = None) -> None:
    """Runs every job concurrently, each on its own interval, until
    `stop_event` is set. A worker crash on graceful shutdown must not leave
    permanent locks or half-finished state (spec Part 6 §111) — jobs run via
    `MemoryRepository`'s own short-lived sessions, so there is no long-held
    resource here to release beyond the asyncio tasks themselves."""
    stop_event = stop_event or asyncio.Event()
    tasks = [asyncio.create_task(_job_loop(job, stop_event), name=job.name) for job in jobs]
    try:
        await asyncio.gather(*tasks)
    finally:
        for task in tasks:
            task.cancel()


def build_default_jobs(settings: Optional[Settings] = None) -> list[ScheduledJob]:
    """Minimal worker set from spec Part 6 §145 (embedding, document sync,
    expiration, reconciliation). Consolidation is deliberately excluded: it
    can create/archive memories and is safer triggered explicitly (CLI/API)
    than run unattended (spec Part 6 §77-78)."""
    settings = settings or get_settings()

    from lib.domain.tasks import document_sync_worker, embedding_worker, expiration_worker, reconciliation_worker
    from lib.presentation.api.deps import get_document_store, get_embedding_provider, get_memory_repo

    memory_repo = get_memory_repo()
    document_store = get_document_store()
    embedding_provider = get_embedding_provider()

    return [
        ScheduledJob(
            "expire_memories", settings.scheduler_expiration_interval_seconds,
            lambda: expiration_worker.run(memory_repo),
        ),
        ScheduledJob(
            "sync_documents", settings.scheduler_document_sync_interval_seconds,
            lambda: document_sync_worker.run(memory_repo, document_store),
        ),
        ScheduledJob(
            "generate_embeddings", settings.scheduler_embedding_interval_seconds,
            lambda: embedding_worker.run(memory_repo, embedding_provider),
        ),
        ScheduledJob(
            "reconcile", settings.scheduler_reconciliation_interval_seconds,
            lambda: reconciliation_worker.run(memory_repo, document_store),
        ),
    ]
