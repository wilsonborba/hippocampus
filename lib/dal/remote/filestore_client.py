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


@dataclass(frozen=True)
class FileStoreUploadResult:
    uri: str
    key: str
    content_type: str
    size_bytes: int
    checksum: str
    filename: str


class FileStoreClient:
    """Adapter to the existing local File Store (FSM).
    Supports stat/existence checks and direct file uploads on behalf of memories."""

    def __init__(
        self,
        base_url: Optional[str],
        api_key: Optional[str] = None,
        app: str = "hippocampus",
        timeout: float = 30.0,
        transport: Optional[httpx.BaseTransport] = None,
    ) -> None:
        self._base_url = base_url.rstrip("/") if base_url else None
        self._api_key = api_key
        self._app = app
        self._timeout = timeout
        self._transport = transport

    @property
    def configured(self) -> bool:
        return self._base_url is not None

    def _headers(self) -> dict[str, str]:
        headers: dict[str, str] = {}
        if self._api_key:
            headers["Authorization"] = f"Bearer {self._api_key}"
        return headers

    def stat(self, uri: str) -> Optional[FileStoreObjectInfo]:
        """Returns None when File Store isn't configured/reachable — callers
        must treat that as "unknown", not "missing" (spec Part 3 §25)."""
        if not self._base_url:
            return None
        try:
            with httpx.Client(timeout=self._timeout, transport=self._transport) as client:
                response = client.head(f"{self._base_url}/{uri.lstrip('/')}", headers=self._headers())
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

    def upload(
        self,
        *,
        album: str,
        filename: str,
        body: bytes,
        content_type: str,
        force: bool = False,
    ) -> FileStoreUploadResult:
        """Uploads file bytes directly to FSM's /{app}/media endpoint."""
        if not self._base_url or not self._api_key:
            from lib.domain.errors import FileStoreNotConfiguredError
            raise FileStoreNotConfiguredError("File Store (FSM) is not configured or missing API key")

        from lib.domain.errors import FileStoreError
        try:
            with httpx.Client(timeout=self._timeout, transport=self._transport) as client:
                response = client.post(
                    f"{self._base_url}/{self._app}/media",
                    headers=self._headers(),
                    data={"album": album, "force": "true" if force else "false"},
                    files={"file": (filename, body, content_type)},
                )
        except httpx.HTTPError as exc:
            logger.error("File Store upload failed: %s", exc)
            raise FileStoreError(f"File Store upload failed: {exc}") from exc

        if response.status_code not in {200, 201}:
            logger.error("File Store rejected upload with status %d: %s", response.status_code, response.text)
            raise FileStoreError(f"File Store rejected upload (status {response.status_code})")

        try:
            payload = response.json()
            item = payload["item"]
            key = str(item["key"])
            checksum = str(item.get("checksum_sha256") or "")
            size_bytes = int(item.get("size_bytes", len(body)))
            ret_content_type = str(item.get("content_type", content_type))
            uri = f"{self._base_url}/{self._app}/media/{key}"
            return FileStoreUploadResult(
                uri=uri,
                key=key,
                content_type=ret_content_type,
                size_bytes=size_bytes,
                checksum=checksum,
                filename=filename,
            )
        except (KeyError, TypeError, ValueError) as exc:
            raise FileStoreError("File Store returned invalid upload response") from exc


def build_default_filestore_client(settings: Optional[Settings] = None) -> FileStoreClient:
    settings = settings or get_settings()
    return FileStoreClient(
        base_url=settings.filestore_url,
        api_key=settings.filestore_api_key,
        app=settings.filestore_app,
        timeout=settings.filestore_timeout_seconds,
    )
