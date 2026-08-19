from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional

import httpx

from lib.core.logs import get_logger
from lib.core.settings import Settings, get_settings

logger = get_logger(__name__)


@dataclass(frozen=True)
class FileStoreObjectInfo:
    uri: str
    content_type: Optional[str] = None
    size_bytes: Optional[int] = None
    checksum: Optional[str] = None
    exists: bool = True


class FileStoreClient:
    """Reference-only adapter to the existing local File Store (spec Part 1
    §10, Part 3 §59-62). Hippocampus never fetches or duplicates raw bytes
    here — it only checks existence/metadata to decide whether a Resource is
    `available`, `missing`, or `unreachable` (spec Part 3 §25)."""

    def __init__(self, base_url: Optional[str], timeout: float = 10.0) -> None:
        self._base_url = base_url.rstrip("/") if base_url else None
        self._timeout = timeout

    @property
    def configured(self) -> bool:
        return self._base_url is not None

    def stat(self, uri: str) -> Optional[FileStoreObjectInfo]:
        """Returns None when File Store isn't configured/reachable — callers
        must treat that as "unknown", not "missing" (spec Part 3 §25)."""
        if not self._base_url:
            return None
        try:
            with httpx.Client(timeout=self._timeout) as client:
                response = client.head(f"{self._base_url}/{uri.lstrip('/')}")
                if response.status_code == 404:
                    return FileStoreObjectInfo(uri=uri, exists=False)
                response.raise_for_status()
                headers = response.headers
                return FileStoreObjectInfo(
                    uri=uri,
                    content_type=headers.get("Content-Type"),
                    size_bytes=int(headers["Content-Length"]) if "Content-Length" in headers else None,
                    checksum=headers.get("ETag", "").strip('"') or None,
                    exists=True,
                )
        except httpx.HTTPError as exc:
            logger.warning("File Store stat failed for uri=%r: %s", uri, exc)
            return None


def build_default_filestore_client(settings: Optional[Settings] = None) -> FileStoreClient:
    settings = settings or get_settings()
    return FileStoreClient(base_url=settings.filestore_url, timeout=settings.filestore_timeout_seconds)
