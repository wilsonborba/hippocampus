from __future__ import annotations

from lib.dal.repositories.tag_repository import normalize_tag_part


def test_lowercases():
    assert normalize_tag_part("Work") == "work"


def test_collapses_separators():
    assert normalize_tag_part("Machine Learning") == "machine-learning"
    assert normalize_tag_part("machine_learning") == "machine-learning"
    assert normalize_tag_part("machine-learning") == "machine-learning"


def test_strips_whitespace():
    assert normalize_tag_part("  work  ") == "work"
