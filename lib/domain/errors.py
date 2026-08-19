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
