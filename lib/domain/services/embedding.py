from __future__ import annotations

import hashlib
import math
from abc import ABC, abstractmethod
from typing import Optional, Sequence

from lib.core.logs import get_logger
from lib.core.settings import Settings, get_settings

logger = get_logger(__name__)


class EmbeddingProvider(ABC):
    """Replaceable AI dependency (spec Part 4 §140, Part 6 §95-97): embedding
    generation must never become a hard requirement for basic memory
    operations. No specific model/dimension is mandated by the spec (Part 7
    §147 — deferred), so this stays a thin interface until one is chosen."""

    name: str = "none"

    @abstractmethod
    def embed(self, text: str) -> Optional[list[float]]: ...

    @property
    @abstractmethod
    def dimensions(self) -> int: ...


class NullEmbeddingProvider(EmbeddingProvider):
    """Default provider: semantic search degrades gracefully (spec Part 4 §55,
    Part 6 §141) rather than being a hard dependency of `remember`/`recall`."""

    name = "none"

    def embed(self, text: str) -> Optional[list[float]]:
        return None

    @property
    def dimensions(self) -> int:
        return 0


class OllamaEmbeddingProvider(EmbeddingProvider):
    """Local embedding model served through Ollama (spec Part 6 §96 —
    infrastructure detail, not a domain requirement; another provider can
    replace this without touching the domain layer)."""

    def __init__(self, base_url: str, model: str, timeout: float = 30.0) -> None:
        self.name = model
        self._base_url = base_url.rstrip("/")
        self._model = model
        self._timeout = timeout
        self._dimensions = 0

    def embed(self, text: str) -> Optional[list[float]]:
        try:
            import httpx

            with httpx.Client(timeout=self._timeout) as client:
                response = client.post(
                    f"{self._base_url}/api/embeddings", json={"model": self._model, "prompt": text}
                )
                response.raise_for_status()
                vector = response.json().get("embedding")
                if not vector:
                    return None
                self._dimensions = len(vector)
                return vector
        except Exception as exc:  # pragma: no cover - depends on external Ollama
            logger.warning("Ollama embedding call failed: %s", exc)
            return None

    @property
    def dimensions(self) -> int:
        return self._dimensions


def build_embedding_provider(settings: Optional[Settings] = None) -> EmbeddingProvider:
    settings = settings or get_settings()
    if settings.embedding_provider == "ollama" and settings.embedding_model:
        return OllamaEmbeddingProvider(
            base_url=settings.ollama_base_url, model=settings.embedding_model
        )
    return NullEmbeddingProvider()


def content_hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def cosine_similarity(a: Sequence[float], b: Sequence[float]) -> float:
    if not a or not b or len(a) != len(b):
        return 0.0
    dot = sum(x * y for x, y in zip(a, b))
    norm_a = math.sqrt(sum(x * x for x in a))
    norm_b = math.sqrt(sum(y * y for y in b))
    if norm_a == 0 or norm_b == 0:
        return 0.0
    return dot / (norm_a * norm_b)
