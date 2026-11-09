from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone

from lib.core.time_utils import ensure_utc
from lib.dal.models import MemoryEventType, MemoryStatus
from lib.dal.repositories.memory_repository import MemoryRepository


@dataclass
class ExpirationReport:
    expired_memory_ids: list[str] = field(default_factory=list)


def run(memory_repo: MemoryRepository, batch_limit: int = 200) -> ExpirationReport:
    """ExpireMemoryWorker (spec Part 4 §68, Part 6 §51-52). Expiration is a
    lifecycle status, not deletion — expired memories remain historically
    inspectable."""
    now = datetime.now(timezone.utc)
    report = ExpirationReport()
    candidates = memory_repo.list(statuses=[MemoryStatus.ACTIVE.value], limit=batch_limit)
    for memory in candidates:
        if memory.expires_at and ensure_utc(memory.expires_at) <= now:
            memory_repo.set_status(memory.id, MemoryStatus.EXPIRED.value)
            memory_repo.add_event(memory.id, MemoryEventType.UPDATED.value, payload={"reason": "expired"})
            report.expired_memory_ids.append(memory.id)
    return report
