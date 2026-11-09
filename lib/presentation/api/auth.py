from __future__ import annotations

import hmac
from dataclasses import dataclass
from typing import Optional

from fastapi import Depends, Header

from lib.core.settings import Settings, get_settings
from lib.domain.errors import ForbiddenError, UnauthorizedError


@dataclass(frozen=True)
class ServiceIdentity:
    """Who's calling (spec Part 7 §8-9 — service identities should be
    distinguishable, not one anonymous system actor)."""

    name: str
    is_admin: bool = False


def get_current_identity(
    authorization: Optional[str] = Header(default=None),
    settings: Settings = Depends(get_settings),
) -> ServiceIdentity:
    """Static bearer-key auth (spec Part 7 §4). An empty `api_keys` config
    means auth is disabled entirely — a valid, spec-sanctioned local/dev
    mode ("a LAN deployment may begin with simplified credentials"), not a
    bug: every request is treated as a single implicit "anonymous" identity
    with full access, matching today's default behavior."""
    if not settings.api_keys:
        return ServiceIdentity(name="anonymous", is_admin=True)

    if not authorization or not authorization.lower().startswith("bearer "):
        raise UnauthorizedError("missing or malformed Authorization header")

    token = authorization.split(" ", 1)[1].strip()
    for service_name, key in settings.api_keys.items():
        if hmac.compare_digest(key, token):
            return ServiceIdentity(name=service_name, is_admin=service_name in settings.admin_services)

    raise UnauthorizedError("invalid API key")


def require_admin(identity: ServiceIdentity = Depends(get_current_identity)) -> ServiceIdentity:
    """Gate for elevated/destructive operations (spec Part 7 §10, e.g. hard
    delete). An AI/agent suggestion is never sufficient authorization by
    itself (spec Part 7 §19) — this only ever checks the caller's own
    verified identity."""
    if not identity.is_admin:
        raise ForbiddenError(f"service {identity.name!r} is not authorized for this operation")
    return identity
