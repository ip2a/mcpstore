"""Cache factory surface for the Rust-backed SDK.

The Python SDK no longer constructs the old Python cache backend. This factory
returns a small configuration view that can be passed to Rust-backed setup code
or inspected by applications that used the historical helper.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional


@dataclass
class CacheStoreConfigView:
    config: Any

    def get_backend_type(self) -> str:
        cache_type = getattr(self.config, "cache_type", None)
        return getattr(cache_type, "value", cache_type) or "memory"

    def get_scope(self) -> str:
        return getattr(self.config, "namespace", None) or "mcpstore"

    def inspect(self) -> dict[str, Any]:
        return {
            "backend": self.get_backend_type(),
            "namespace": self.get_scope(),
            "config": self.config,
            "entities": [],
            "relations": [],
            "states": [],
            "events": [],
        }

    def health_check(self) -> dict[str, Any]:
        return {
            "backend": self.get_backend_type(),
            "namespace": self.get_scope(),
            "healthy": True,
        }

    def stats(self) -> dict[str, Any]:
        return self.inspect()

    def dump_all(self) -> dict[str, Any]:
        return self.inspect()

    def read_entity(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> list[dict[str, Any]]:
        return []

    def read_relation(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> list[dict[str, Any]]:
        return []

    def read_state(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> list[dict[str, Any]]:
        return []


def create_kv_store(cache_config: Any) -> CacheStoreConfigView:
    return CacheStoreConfigView(cache_config)
