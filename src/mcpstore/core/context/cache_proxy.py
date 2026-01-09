"""
CacheProxy - 只读缓存代理

为 pykv 三层缓存提供统一的只读访问接口，支持全局/Agent/Service/Tool 视角过滤。
"""

import asyncio
import logging
from datetime import datetime
from typing import Any, Dict, List, Optional, Sequence

logger = logging.getLogger(__name__)


class CacheProxy:
    """只读缓存代理，按层提供统一读取接口。"""

    def __init__(self, context: "MCPStoreContext", scope: str = "global", scope_value: Optional[str] = None):
        self._context = context
        self._scope = scope  # global | agent | service | tool
        self._scope_value = scope_value
        registry = getattr(context._store, "registry", None)
        self._cache_layer = None
        if registry:
            # 优先使用新版 cache_layer_manager
            self._cache_layer = getattr(registry, "_cache_layer_manager", None) or getattr(registry, "_cache_layer", None)
            logger.debug(
                "[CACHE_PROXY_INIT] scope=%s value=%s cache_layer=%s namespace=%s",
                scope,
                scope_value,
                self._cache_layer.__class__.__name__ if self._cache_layer else None,
                getattr(self._cache_layer, '_namespace', None) if self._cache_layer else None,
            )
        self._bridge = getattr(context, "_bridge", None)

    # === 对外方法（同步）===
    def read_entity(self, entity_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        return self._run_async(self.read_entity_async(entity_type, key), "cache_proxy.read_entity")

    def read_relation(self, relation_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        return self._run_async(self.read_relation_async(relation_type, key), "cache_proxy.read_relation")

    def read_state(self, state_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        return self._run_async(self.read_state_async(state_type, key), "cache_proxy.read_state")

    def dump_all(self) -> Dict[str, Any]:
        return self._run_async(self.dump_all_async(), "cache_proxy.dump_all")

    def inspect(self) -> Dict[str, Any]:
        return self._run_async(self.inspect_async(), "cache_proxy.inspect")

    # === 异步实现 ===
    async def read_entity_async(self, entity_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        types = self._resolve_types(entity_type, default=["services", "tools", "agents", "store", "clients"])
        return await self._read_layer(types, key, layer="entity")

    async def read_relation_async(self, relation_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        types = self._resolve_types(relation_type, default=["agent_services", "service_tools"])
        return await self._read_layer(types, key, layer="relations")

    async def read_state_async(self, state_type: Optional[Any] = None, key: Optional[str] = None) -> List[Dict[str, Any]]:
        types = self._resolve_types(state_type, default=["service_status", "service_metadata"])
        return await self._read_layer(types, key, layer="state")

    async def dump_all_async(self) -> Dict[str, Any]:
        entities, relations, states = await asyncio.gather(
            self.read_entity_async(),
            self.read_relation_async(),
            self.read_state_async(),
        )
        return {
            "entities": entities,
            "relations": relations,
            "states": states,
            "metadata": {
                "namespace": getattr(self._cache_layer, "_namespace", None),
                "backend": self.get_backend_type(),
                "scope": self.get_scope(),
                "exported_at": datetime.utcnow().isoformat(),
            },
        }

    async def inspect_async(self) -> Dict[str, Any]:
        entities = await self.read_entity_async()
        relations = await self.read_relation_async()
        states = await self.read_state_async()

        def _count_by_type(items: List[Dict[str, Any]]) -> Dict[str, int]:
            counts: Dict[str, int] = {}
            for item in items:
                t = item.get("_type", "unknown")
                counts[t] = counts.get(t, 0) + 1
            return counts

        return {
            "backend": self.get_backend_type(),
            "namespace": getattr(self._cache_layer, "_namespace", None),
            "scope": self.get_scope(),
            "counts": {
                "entities": _count_by_type(entities),
                "relations": _count_by_type(relations),
                "states": _count_by_type(states),
            },
            "collections": sorted({item.get("_collection") for item in (entities + relations + states) if item.get("_collection")}),
            "entities": entities,
            "relations": relations,
            "states": states,
        }

    # === 辅助方法 ===
    def get_backend_type(self) -> str:
        kv = getattr(self._cache_layer, "_kv_store", None)
        return kv.__class__.__name__ if kv else "unknown"

    def get_scope(self) -> str:
        if self._scope == "global":
            return "global"
        if self._scope_value:
            return f"{self._scope}:{self._scope_value}"
        return self._scope

    def _resolve_types(self, value: Optional[Any], default: List[str]) -> List[str]:
        if value is None:
            return default
        if isinstance(value, str):
            return [value]
        if isinstance(value, (list, tuple, set)):
            return list(value)
        return default

    def _wrap_result(self, type_name: str, key: str, data: Dict[str, Any], collection: str) -> Dict[str, Any]:
        wrapped = {"_key": key, "_type": type_name, "_collection": collection}
        if isinstance(data, dict):
            wrapped.update(data)
        return wrapped

    async def _await_safe(self, coro, op_name: str):
        """
        在存在 AOB 时使用桥接执行，避免跨事件循环的 Future 绑定错误。
        """
        try:
            if hasattr(self._context, "bridge_execute"):
                return await self._context.bridge_execute(coro, op_name=op_name)
        except Exception:
            # fallback to direct await
            pass
        return await coro

    def _apply_scope_filter(self, items: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        if self._scope == "global":
            return items
        target = self._scope_value
        if self._scope == "agent":
            return [it for it in items if it.get("agent_id") == target or it.get("_key", "").startswith(f"{target}")]
        if self._scope == "service":
            return [
                it for it in items
                if it.get("service_global_name") == target
                or it.get("service_original_name") == target
                or it.get("service_name") == target
                or it.get("_key") == target
            ]
        if self._scope == "tool":
            return [
                it for it in items
                if it.get("tool_global_name") == target
                or it.get("tool_original_name") == target
                or it.get("name") == target
                or it.get("_key") == target
            ]
        return items

    async def _read_layer(self, types: Sequence[str], key: Optional[str], layer: str) -> List[Dict[str, Any]]:
        if not self._cache_layer:
            logger.warning("[CACHE] Cache layer not available; returning empty list.")
            return []

        results: List[Dict[str, Any]] = []
        logger.debug(
            "[CACHE_PROXY] start read layer=%s types=%s key=%s scope=%s value=%s cache_ns=%s backend=%s",
            layer,
            list(types),
            key,
            self._scope,
            self._scope_value,
            getattr(self._cache_layer, "_namespace", None) if self._cache_layer else None,
            getattr(getattr(self._cache_layer, "_kv_store", None), "__class__", type("X",(object,),{})).__name__ if self._cache_layer else None,
        )
        for t in types:
            try:
                if layer == "entity":
                    collection = self._cache_layer._get_entity_collection(t)
                    if key:
                        data = await self._await_safe(self._cache_layer.get_entity(t, key), f"cache.read_entity.{t}")
                        if data is not None:
                            results.append(self._wrap_result(t, key, data, collection))
                    else:
                        all_data = await self._await_safe(self._cache_layer.get_all_entities_async(t), f"cache.read_entities.{t}")
                        for k, v in all_data.items():
                            results.append(self._wrap_result(t, k, v, collection))
                elif layer == "relations":
                    collection = self._cache_layer._get_relation_collection(t)
                    if key:
                        data = await self._await_safe(self._cache_layer.get_relation(t, key), f"cache.read_relation.{t}")
                        if data is not None:
                            results.append(self._wrap_result(t, key, data, collection))
                    else:
                        all_data = await self._await_safe(self._cache_layer.get_all_relations_async(t), f"cache.read_relations.{t}")
                        for k, v in all_data.items():
                            results.append(self._wrap_result(t, k, v, collection))
                elif layer == "state":
                    collection = self._cache_layer._get_state_collection(t)
                    if key:
                        data = await self._await_safe(self._cache_layer.get_state(t, key), f"cache.read_state.{t}")
                        if data is not None:
                            results.append(self._wrap_result(t, key, data, collection))
                    else:
                        all_data = await self._await_safe(self._cache_layer.get_all_states_async(t), f"cache.read_states.{t}")
                        for k, v in all_data.items():
                            results.append(self._wrap_result(t, k, v, collection))
            except Exception as e:
                logger.warning(f"[CACHE] read_{layer} failed for type={t}, key={key}: {e}")

        filtered = self._apply_scope_filter(results)
        logger.debug(
            "[CACHE_PROXY] done layer=%s count=%s filtered=%s scope=%s value=%s",
            layer, len(results), len(filtered), self._scope, self._scope_value
        )
        return filtered

    def _run_async(self, coro, op_name: str):
        if self._bridge:
            return self._bridge.run(coro, op_name=op_name)
        return asyncio.run(coro)


# 延迟导入用于类型检查
from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from .base_context import MCPStoreContext
