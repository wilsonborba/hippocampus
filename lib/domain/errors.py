from __future__ import annotations


class DomainError(Exception):
    """Base for errors the presentation layer (API/CLI) must translate into a
    machine-readable response (spec Part 5 §7-8) instead of leaking a raw
    exception/traceback. `status_code` centralizes the HTTP mapping (spec
    Part 5 §9) here instead of scattering if/elif chains through routes."""

    code = "domain_error"
    status_code = 400


class MemoryNotFoundError(DomainError):
    code = "memory_not_found"
    status_code = 404


class TagNotFoundError(DomainError):
    code = "tag_not_found"
    status_code = 404


class EntityNotFoundError(DomainError):
    code = "entity_not_found"
    status_code = 404


class ResourceNotFoundError(DomainError):
    code = "resource_not_found"
    status_code = 404


class RelationshipNotFoundError(DomainError):
    code = "relationship_not_found"
    status_code = 404


class ValidationError(DomainError):
    code = "invalid_request"
    status_code = 400


class InvalidRelationshipError(DomainError):
    """Semantic relationship validation failure (spec Part 5 §131) — e.g. a
    memory relating to itself — kept distinct from a raw DB constraint error."""

    code = "invalid_relationship"
    status_code = 422


class InvalidMemoryStatusError(DomainError):
    code = "invalid_memory_status"
    status_code = 422


class UnauthorizedError(DomainError):
    """Missing/invalid credentials (spec Part 7 §4-6). Reused by the API auth
    dependency; it's not a memory-domain concern, but it shares the same
    machine-readable error envelope, so it lives alongside the other errors
    that already map through `app.py`'s exception handler."""

    code = "unauthorized"
    status_code = 401


class ForbiddenError(DomainError):
    """Authenticated but not authorized for this operation (spec Part 7 §7,
    §10 — e.g. hard delete requires an elevated service identity)."""

    code = "forbidden"
    status_code = 403


class UnsupportedRenderFormatError(DomainError):
    """`GET /memories/{id}/graph?format=...` requested a format the graph
    renderer doesn't know (see `lib.domain.rendering`)."""

    code = "unsupported_render_format"
    status_code = 400


class FileStoreError(DomainError):
    code = "filestore_error"
    status_code = 502


class FileStoreNotConfiguredError(DomainError):
    code = "filestore_not_configured"
    status_code = 503
