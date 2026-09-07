from __future__ import annotations

import pytest

from lib.dal.models import MemoryStatus
from lib.domain.errors import InvalidRelationshipError, MemoryNotFoundError, ValidationError
from lib.domain.models import MemoryInput


def test_remember_requires_some_content(memory_service):
    with pytest.raises(ValidationError):
        memory_service.remember(MemoryInput())


def test_remember_creates_active_memory_with_provenance(memory_service):
    memory = memory_service.remember(
        MemoryInput(content="CouchDB is the selected document store.", memory_type="decision")
    )
    assert memory.status == MemoryStatus.ACTIVE.value
    assert memory.memory_type == "decision"
    provenance = memory_service.provenance(memory.id)
    assert len(provenance) == 1
    assert provenance[0].actor_type == "user"


def test_remember_rejects_unknown_memory_type(memory_service):
    with pytest.raises(ValidationError):
        memory_service.remember(MemoryInput(content="x", memory_type="not-a-real-type"))


def test_get_missing_memory_raises(memory_service):
    with pytest.raises(MemoryNotFoundError):
        memory_service.get("does-not-exist")


def test_tag_normalizes_and_deduplicates(memory_service):
    memory = memory_service.remember(MemoryInput(content="x"))
    memory_service.tag(memory.id, ["Project:Hippocampus"])
    memory_service.tag(memory.id, ["project:hippocampus"])  # same tag again
    tags = memory_service.tags_for(memory.id)
    assert len(tags) == 1
    assert tags[0].canonical_name == "project:hippocampus"


def test_untag_removes_association_not_definition(memory_service, tag_repo):
    memory = memory_service.remember(MemoryInput(content="x", tags=["context:work"]))
    tag = memory_service.tags_for(memory.id)[0]
    assert memory_service.untag(memory.id, tag.id) is True
    assert memory_service.tags_for(memory.id) == []
    assert tag_repo.get_by_id(tag.id) is not None  # the Tag definition itself survives


def test_link_rejects_self_relationship(memory_service):
    memory = memory_service.remember(MemoryInput(content="x"))
    with pytest.raises(InvalidRelationshipError):
        memory_service.link(memory.id, "related_to", memory.id)


def test_link_rejects_unknown_relation_type(memory_service):
    a = memory_service.remember(MemoryInput(content="a"))
    b = memory_service.remember(MemoryInput(content="b"))
    with pytest.raises(ValidationError):
        memory_service.link(a.id, "not_a_real_relation", b.id)


def test_supersede_preserves_old_memory_and_marks_status(memory_service):
    old = memory_service.remember(MemoryInput(content="MongoDB selected."))
    new = memory_service.remember(MemoryInput(content="CouchDB selected."))
    memory_service.supersede(old.id, new.id)

    refreshed_old = memory_service.get(old.id)
    refreshed_new = memory_service.get(new.id)
    assert refreshed_old.status == MemoryStatus.SUPERSEDED.value
    assert refreshed_new.status == MemoryStatus.ACTIVE.value
    # the old memory's content must survive untouched (spec Part 2 §75)
    assert refreshed_old.content == "MongoDB selected."

    relationships = memory_service.neighbors(new.id)
    assert any(r.relation_type == "supersedes" and r.target_memory_id == old.id for r in relationships)


def test_forget_is_logical_not_physical(memory_service):
    memory = memory_service.remember(MemoryInput(content="temporary detail"))
    memory_service.forget(memory.id)
    forgotten = memory_service.get(memory.id, touch=False)
    assert forgotten.status == MemoryStatus.FORGOTTEN.value
    # provenance/history must remain inspectable after a logical forget
    assert memory_service.provenance(memory.id)


def test_hard_delete_excludes_from_get(memory_service):
    memory = memory_service.remember(MemoryInput(content="to be deleted"))
    memory_service.hard_delete(memory.id)
    with pytest.raises(MemoryNotFoundError):
        memory_service.get(memory.id)


def test_update_rejects_core_claim_fields(memory_service):
    memory = memory_service.remember(MemoryInput(content="original claim"))
    with pytest.raises(ValidationError):
        memory_service.update(memory.id, {"content": "rewritten claim"})


def test_update_allows_safe_fields(memory_service):
    memory = memory_service.remember(MemoryInput(content="x"))
    updated = memory_service.update(memory.id, {"importance": 0.9})
    assert updated.importance == 0.9


def test_reinforce_is_distinct_from_access_count(memory_service):
    memory = memory_service.remember(MemoryInput(content="x"))
    memory_service.get(memory.id, touch=True)
    memory_service.get(memory.id, touch=True)
    reinforced = memory_service.reinforce(memory.id)
    assert reinforced.access_count == 2
    assert reinforced.reinforcement_count == 1


def test_upload_and_associate_resource_success(memory_repo, tag_repo, entity_repo, resource_repo, document_store):
    import httpx
    from lib.dal.remote.filestore_client import FileStoreClient
    from lib.domain.services.memory_service import MemoryService

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(
            status_code=200,
            json={
                "item": {
                    "key": "mem-upload-key-1",
                    "checksum_sha256": "sha256checksum",
                    "size_bytes": 13,
                    "content_type": "text/plain",
                }
            },
        )

    filestore = FileStoreClient(
        base_url="http://filestore.local",
        api_key="secret",
        transport=httpx.MockTransport(handler),
    )
    service = MemoryService(
        memory_repo=memory_repo,
        tag_repo=tag_repo,
        entity_repo=entity_repo,
        resource_repo=resource_repo,
        document_store=document_store,
        filestore_client=filestore,
    )
    memory = service.remember(MemoryInput(content="meeting notes"))
    resource = service.upload_and_associate_resource(
        memory_id=memory.id,
        filename="notes.txt",
        content=b"hello content",
        content_type="text/plain",
        relationship="references",
        title="Notes Document",
    )
    assert resource.ownership == "hippocampus"
    assert resource.status == "available"
    assert resource.source_system == "fsm"
    assert resource.resource_type == "file"
    assert resource.external_id == "mem-upload-key-1"
    assert resource.uri == "http://filestore.local/hippocampus/media/mem-upload-key-1"
    assert resource.checksum == "sha256checksum"
    assert resource.size_bytes == 13
    assert resource.title == "Notes Document"


def test_upload_and_associate_resource_without_filestore_raises(memory_service):
    from lib.domain.errors import FileStoreNotConfiguredError

    memory = memory_service.remember(MemoryInput(content="notes"))
    with pytest.raises(FileStoreNotConfiguredError):
        memory_service.upload_and_associate_resource(
            memory_id=memory.id,
            filename="notes.txt",
            content=b"content",
            content_type="text/plain",
        )

