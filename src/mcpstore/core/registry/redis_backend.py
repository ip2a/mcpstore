from __future__ import annotations

import json
from typing import Optional, Dict, Any, List

from .cache_backend import CacheBackend


from .key_builder import KeyBuilder


class RedisCacheBackend(CacheBackend):
    """
    Redis-based cache backend (code skeleton; client deferred).

    Intent
    - No external dependency or I/O in this module
    - Default remains Memory; this backend activates only when configured and a client is attached

    Scope notes
    - Current implementation namespaces by agent_id only: {base}:agent:{agent_id}:...
    - For future multi-scope (device/user/pub), compose keys via KeyBuilder.scope() and
      apply read precedence device > user > pub using ScopeResolver.get_read_order(ctx).
      Storage (this file) remains decoupled from policy (ScopeResolver).

    Key layout (agent namespacing)
    - Agent clients set:          {base}:agent:{agent_id}:clients
    - Client config string(JSON): {base}:client:{client_id}:config
    - Service→Client hash:        {base}:agent:{agent_id}:service_to_client
    - Tool→Service hash:          {base}:agent:{agent_id}:tool_to_service
    - Tool definition string:     {base}:agent:{agent_id}:tool:{tool_name}:def

    TTL / size controls
    - To be configured at integration time via extras package; no TTL applied here.
    """

    def __init__(self, key_builder: Optional[KeyBuilder] = None, normalizer=None) -> None:
        self.key_builder = key_builder or KeyBuilder()
        self._redis = None  # client attached later
        self._normalizer = normalizer  # optional tool normalizer
    # --- Optional: future multi-scope helpers (not used yet) ---
    def _k_scope_base(self, scope: str, owner: str) -> str:
        # Example: f"{self.key_builder.scope(scope, owner)}"
        return self.key_builder.scope(scope, owner)

    def _k_scope_entity(self, scope: str, owner: str, entity: str, identifier: str) -> str:
        # Example: f"{self.key_builder.entity(scope, owner, entity, identifier)}"
        return self.key_builder.entity(scope, owner, entity, identifier)

    # --- lifecycle ---
    def _t(self):
        """Return active write target: pipeline if open, else client. None if no client."""
        if getattr(self, "_pipe", None) is not None:
            return self._pipe
        return getattr(self, "_redis", None)

    def attach_client(self, client: Any) -> None:  # type: ignore[name-defined]
        """Attach a redis-like client with `sadd/srem/smembers/set/get/hset/hget/hdel/delete/scan_iter`."""
        self._redis = client

    # ---- Optional transaction & health ----
    def begin(self) -> None:
        """Open a transactional pipeline if client supports it."""
        if getattr(self, "_redis", None) is None:
            return
        pipe_factory = getattr(self._redis, "pipeline", None)
        if callable(pipe_factory):
            self._pipe = pipe_factory(transaction=True)

    def commit(self) -> None:
        p = getattr(self, "_pipe", None)
        if p is None:
            return
        try:
            exec_fn = getattr(p, "execute", None)
            if callable(exec_fn):
                exec_fn()
        finally:
            self._pipe = None

    def rollback(self) -> None:
        p = getattr(self, "_pipe", None)
        if p is None:
            return
        try:
            discard = getattr(p, "reset", None)
            if callable(discard):
                discard()
            else:
                # best-effort close
                close = getattr(p, "close", None)
                if callable(close):
                    close()
        finally:
            self._pipe = None

    def health_check(self) -> bool:
        c = getattr(self, "_redis", None)
        if c is None:
            return False
        try:
            ping = getattr(c, "ping", None)
            if callable(ping):
                res = ping()
                return bool(res) if not isinstance(res, (bytes, bytearray)) else True
            return True
        except Exception:
            return False

    # ---- key helpers (agent-partitioned) ----
    def _k_agent_clients(self, agent_id: str) -> str:
        return f"{self.key_builder.base()}:agent:{agent_id}:clients"

    def _k_client_config(self, client_id: str) -> str:
        return f"{self.key_builder.base()}:client:{client_id}:config"

    def _k_service_to_client(self, agent_id: str) -> str:
        return f"{self.key_builder.base()}:agent:{agent_id}:service_to_client"

    def _k_tool_to_service(self, agent_id: str) -> str:
        return f"{self.key_builder.base()}:agent:{agent_id}:tool_to_service"

    # ---- Client/Service mappings ----
    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        t = self._t()
        if t is None:
            return
        t.sadd(self._k_agent_clients(agent_id), client_id)

    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        t = self._t()
        if t is None:
            return
        t.srem(self._k_agent_clients(agent_id), client_id)

    def get_agent_clients_from_cache(self, agent_id: str) -> List[str]:
        t = self._t()
        if t is None:
            return []
        members = t.smembers(self._k_agent_clients(agent_id)) or []
        return sorted([m.decode("utf-8") if isinstance(m, (bytes, bytearray)) else str(m) for m in members])

    def add_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        t = self._t()
        if t is None:
            return
        t.set(self._k_client_config(client_id), json.dumps(config, ensure_ascii=False))

    def update_client_config(self, client_id: str, updates: Dict[str, Any]) -> None:
        t = self._t()
        if t is None:
            return
        key = self._k_client_config(client_id)
        current_raw = self._redis.get(key) if getattr(self, "_redis", None) is not None else None
        current: Dict[str, Any] = {}
        if current_raw:
            try:
                if isinstance(current_raw, (bytes, bytearray)):
                    current = json.loads(current_raw.decode("utf-8"))
                else:
                    current = json.loads(current_raw)
            except Exception:
                current = {}
        current.update(updates)
        self._redis.set(key, json.dumps(current, ensure_ascii=False))

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        if getattr(self, "_redis", None) is None:
            return None
        raw = self._redis.get(self._k_client_config(client_id))
        if not raw:
            return None
        try:
            if isinstance(raw, (bytes, bytearray)):
                return json.loads(raw.decode("utf-8"))
            return json.loads(raw)
        except Exception:
            return None

    def remove_client_config(self, client_id: str) -> None:
        t = self._t()
        if t is None:
            return
        t.delete(self._k_client_config(client_id))

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        t = self._t()
        if t is None:
            return
        t.hset(self._k_service_to_client(agent_id), service_name, client_id)

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        if getattr(self, "_redis", None) is None:
            return None
        val = self._redis.hget(self._k_service_to_client(agent_id), service_name)
        if val is None:
            return None
        return val.decode("utf-8") if isinstance(val, (bytes, bytearray)) else str(val)

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        t = self._t()
        if t is None:
            return
        t.hdel(self._k_service_to_client(agent_id), service_name)

    # ---- Tools mapping ----
    def map_tool_to_service(self, agent_id: str, tool_name: str, service_name: str) -> None:
        t = self._t()
        if t is None:
            return
        t.hset(self._k_tool_to_service(agent_id), tool_name, service_name)

    def unmap_tool(self, agent_id: str, tool_name: str) -> None:
        t = self._t()
        if t is None:
            return
        t.hdel(self._k_tool_to_service(agent_id), tool_name)

    # ---- Tool definitions (optional full mode) ----
    def _k_tool_def(self, agent_id: str, tool_name: str) -> str:
        return f"{self.key_builder.base()}:agent:{agent_id}:tool:{tool_name}:def"

    def upsert_tool_def(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:  # type: ignore[name-defined]
        t = self._t()
        if t is None:
            return
        normalized = self._normalizer.normalize_tool(tool_name, tool_def) if self._normalizer else tool_def
        t.set(self._k_tool_def(agent_id, tool_name), json.dumps(normalized, ensure_ascii=False))

    def delete_tool_def(self, agent_id: str, tool_name: str) -> None:
        t = self._t()
        if t is None:
            return
        t.delete(self._k_tool_def(agent_id, tool_name))

    def get_tool_def(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:
        if getattr(self, "_redis", None) is None:
            return None
        raw = self._redis.get(self._k_tool_def(agent_id, tool_name))
        if not raw:
            return None
        try:
            if isinstance(raw, (bytes, bytearray)):
                return json.loads(raw.decode("utf-8"))
            return json.loads(raw)
        except Exception:
            return None

    def list_tool_names(self, agent_id: str) -> List[str]:  # type: ignore[name-defined]
        t = self._t()
        if t is None:
            return []
        keys = t.hkeys(self._k_tool_to_service(agent_id)) or []
        return sorted([k.decode("utf-8") if isinstance(k, (bytes, bytearray)) else str(k) for k in keys])

    # ---- Session (optional in M2): not persisted in Redis skeleton ----
    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:  # type: ignore[name-defined]
        return

    def get_session(self, agent_id: str, service_name: str):  # -> Optional[Any]
        return None

    # ---- Bulk ----
    def clear_agent(self, agent_id: str) -> None:
        t = getattr(self, "_redis", None)
        if t is None:
            return
        base = f"{self.key_builder.base()}:agent:{agent_id}:"
        for k in t.scan_iter(match=base + "*"):
            t.delete(k)


