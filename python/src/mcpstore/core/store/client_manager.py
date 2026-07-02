"""Compatibility client manager for the Rust-backed store package."""

from __future__ import annotations

from typing import Optional


class ClientManager:
    """Legacy data-space identifier holder.

    The Rust-backed architecture no longer uses Python client managers for
    service ownership or persistence. This class remains as a data-only import
    compatibility object for code that reads ``global_agent_store_id``.
    """

    def __init__(self, global_agent_store_id: Optional[str] = None):
        self.global_agent_store_id = global_agent_store_id or self._generate_data_space_client_id()

    def _generate_data_space_client_id(self) -> str:
        return "global_agent_store"


__all__ = ["ClientManager"]
