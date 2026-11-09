from __future__ import annotations

import httpx
import pytest

from lib.dal.remote.filestore_client import FileStoreClient
from lib.domain.errors import FileStoreError, FileStoreNotConfiguredError


def test_filestore_client_stat_not_configured():
    client = FileStoreClient(base_url=None)
    assert client.configured is False
    assert client.stat("some/uri") is None


def test_filestore_client_upload_not_configured():
    client = FileStoreClient(base_url=None)
    with pytest.raises(FileStoreNotConfiguredError):
        client.upload(album="test", filename="file.txt", body=b"hello", content_type="text/plain")


def test_filestore_client_upload_success():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        assert request.url.path == "/hippocampus/media"
        assert request.headers.get("Authorization") == "Bearer test-key"
        return httpx.Response(
            status_code=200,
            json={
                "item": {
                    "key": "test-key-123",
                    "checksum_sha256": "abcdef123456",
                    "size_bytes": 5,
                    "content_type": "text/plain",
                }
            },
        )

    transport = httpx.MockTransport(handler)
    client = FileStoreClient(
        base_url="http://filestore.local",
        api_key="test-key",
        app="hippocampus",
        transport=transport,
    )
    result = client.upload(
        album="test-album",
        filename="hello.txt",
        body=b"hello",
        content_type="text/plain",
    )
    assert result.key == "test-key-123"
    assert result.uri == "http://filestore.local/hippocampus/media/test-key-123"
    assert result.checksum == "abcdef123456"
    assert result.size_bytes == 5
    assert result.content_type == "text/plain"


def test_filestore_client_upload_http_error():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(status_code=500, text="Internal server error")

    transport = httpx.MockTransport(handler)
    client = FileStoreClient(
        base_url="http://filestore.local",
        api_key="test-key",
        transport=transport,
    )
    with pytest.raises(FileStoreError):
        client.upload(
            album="test-album",
            filename="hello.txt",
            body=b"hello",
            content_type="text/plain",
        )
