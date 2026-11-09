from __future__ import annotations

import asyncio

import pytest

from lib.domain.tasks.scheduler import ScheduledJob, run_scheduler


@pytest.mark.asyncio
async def test_scheduler_runs_each_job_at_its_own_interval():
    calls = {"fast": 0, "slow": 0}

    def fast_job() -> None:
        calls["fast"] += 1

    def slow_job() -> None:
        calls["slow"] += 1

    jobs = [
        ScheduledJob("fast", 0.02, fast_job),
        ScheduledJob("slow", 10.0, slow_job),  # long enough to only fire once
    ]
    stop_event = asyncio.Event()

    async def _stop_soon() -> None:
        await asyncio.sleep(0.12)
        stop_event.set()

    await asyncio.gather(run_scheduler(jobs, stop_event), _stop_soon())

    assert calls["fast"] >= 3  # fired repeatedly on its short interval
    assert calls["slow"] == 1  # fired once immediately, interval never elapsed


@pytest.mark.asyncio
async def test_a_failing_job_does_not_stop_other_jobs():
    calls = {"healthy": 0}

    def failing_job() -> None:
        raise RuntimeError("boom")

    def healthy_job() -> None:
        calls["healthy"] += 1

    jobs = [ScheduledJob("failing", 0.02, failing_job), ScheduledJob("healthy", 0.02, healthy_job)]
    stop_event = asyncio.Event()

    async def _stop_soon() -> None:
        await asyncio.sleep(0.08)
        stop_event.set()

    await asyncio.gather(run_scheduler(jobs, stop_event), _stop_soon())

    assert calls["healthy"] >= 2
