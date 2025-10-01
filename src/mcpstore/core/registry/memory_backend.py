# 缓存重构（支持 Redis）计划单
#
# 版本：v0.1（草案，后续将持续更新）
#
# ## 目标（满足你的三点诉求）
# - 架构与规则清晰：形成一份“缓存架构参考手册”，为现在与未来新增缓存实现提供统一标准。
# - 操作方法清晰：提供统一的缓存操作门面与接口规范，所有读写从统一入口经过，禁止直连底层字典。
# - 可插拔后端：默认内存实现（兼容现状）；可选 Redis 配置即启用远程共享缓存；未来可扩展到更多后端，用户只需在 Store 初始化时传入配置（可传可不传）。
#
# ## 当前现状（摘要）
# - Registry 为缓存 SSoT（按 agent_id 隔离）：
#   - sessions、tool_cache、tool_to_session_map
#   - service_states、service_metadata
#   - agent_clients、client_configs、service_to_client
#   - agent_to_global_mappings / global_to_agent_mappings
# - 写路径：缓存优先（立即可见）→ 生命周期连接（异步）→ 持久化（mcp.json 单一数据源）。
# - 读路径：纯缓存只读；SmartCacheQuery 提供过滤/排序。
# - 事务：CacheTransactionManager 通过全量快照回滚。
# - 问题点：部分模块直访 Registry 内部字典；无显式写入原子区；tool→service 兜底靠“前缀启发式”。
#
# ## 目标架构（分层）
# 1) 门面层（Facade）：
#    - 提供统一的缓存操作入口（如 CacheService / RegistryRepository）。
#    - 屏蔽调用方对底层结构与后端细节的感知。
# 2) 领域层（Registry）：
#    - 保留领域逻辑（映射维护、状态衍生、校验），但通过“存储后端接口”读写。
# 3) 存储后端接口（CacheBackend / IRegistryStore）：
#    - 定义标准 CRUD 与批量/事务操作。
#    - 实现：MemoryBackend（默认）、RedisBackend（可选）。
# 4) 并发控制：
#    - per-agent 原子写区（asyncio.Lock）。
#    - 后端事务：Redis 使用 MULTI/EXEC 或 Lua 保证原子性。
# 5) 本地热点缓存（可选）：
#    - 小容量 LRU（进程内），配合失效通知（Redis Pub/Sub 或 Keyspace Notifications）。
#
# ## 数据域与后端映射建议
# - 仅内存（不适合 Redis）
#   - sessions（不可序列化/无共享价值）
# - 优先放入 Redis 的域（支持共享、多进程）：
#   - service_states、service_metadata
#   - agent_clients、client_configs、service_to_client
#   - tool-to-service 硬映射（替代当前前缀启发式）
# - 按需（权衡大小/频次）
#   - tool_cache（通常较大，可考虑仅索引入 Redis，工具详情仍走内存+惰性拉取）
#
# ## 统一接口草案（不改代码，仅规范提议）
# - CacheBackend 接口（示例）

from __future__ import annotations

from typing import Optional, Dict, Any, List, TYPE_CHECKING

from .cache_backend import CacheBackend

if TYPE_CHECKING:  # avoid runtime circular import
    from .core_registry import ServiceRegistry


class MemoryCacheBackend(CacheBackend):
    """In-memory backend that directly manipulates ServiceRegistry's dict state.

    This is a thin adapter around the current in-memory data structures, enabling
    the registry to depend on an abstract backend interface without changing
    external behavior.
    """

    def __init__(self, registry: 'ServiceRegistry') -> None:
        self.registry = registry

    # ---- Client/Service mappings ----
    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        if agent_id not in self.registry.agent_clients:
            self.registry.agent_clients[agent_id] = []
        if client_id not in self.registry.agent_clients[agent_id]:
            self.registry.agent_clients[agent_id].append(client_id)

    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        if agent_id in self.registry.agent_clients and client_id in self.registry.agent_clients[agent_id]:
            self.registry.agent_clients[agent_id].remove(client_id)
            if not self.registry.agent_clients[agent_id]:
                del self.registry.agent_clients[agent_id]

    def get_agent_clients_from_cache(self, agent_id: str) -> List[str]:
        return self.registry.agent_clients.get(agent_id, [])

    def add_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        self.registry.client_configs[client_id] = config

    def update_client_config(self, client_id: str, updates: Dict[str, Any]) -> None:
        if client_id in self.registry.client_configs:
            self.registry.client_configs[client_id].update(updates)
        else:
            self.registry.client_configs[client_id] = updates

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        return self.registry.client_configs.get(client_id)

    def remove_client_config(self, client_id: str) -> None:
        self.registry.client_configs.pop(client_id, None)

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        if agent_id not in self.registry.service_to_client:
            self.registry.service_to_client[agent_id] = {}
        self.registry.service_to_client[agent_id][service_name] = client_id

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        return self.registry.service_to_client.get(agent_id, {}).get(service_name)

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        if agent_id in self.registry.service_to_client:
            self.registry.service_to_client[agent_id].pop(service_name, None)

    # ---- Tools mapping ----
    def map_tool_to_service(self, agent_id: str, tool_name: str, service_name: str) -> None:
        if agent_id not in self.registry.tool_to_service:
            self.registry.tool_to_service[agent_id] = {}
        self.registry.tool_to_service[agent_id][tool_name] = service_name

    def unmap_tool(self, agent_id: str, tool_name: str) -> None:
        if agent_id in self.registry.tool_to_service:
            self.registry.tool_to_service[agent_id].pop(tool_name, None)

    # ---- Tool definitions (optional full mode) ----
    def upsert_tool_def(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:  # type: ignore[name-defined]
        if agent_id not in self.registry.tool_cache:
            self.registry.tool_cache[agent_id] = {}
        self.registry.tool_cache[agent_id][tool_name] = tool_def

    def delete_tool_def(self, agent_id: str, tool_name: str) -> None:
        if agent_id in self.registry.tool_cache:
            self.registry.tool_cache[agent_id].pop(tool_name, None)

    def get_tool_def(self, agent_id: str, tool_name: str):  # -> Optional[Dict[str, Any]]
        return self.registry.tool_cache.get(agent_id, {}).get(tool_name)

    def list_tool_names(self, agent_id: str) -> List[str]:  # type: ignore[name-defined]
        return sorted(list(self.registry.tool_cache.get(agent_id, {}).keys()))

    # ---- Optional: session ----
    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:  # type: ignore[name-defined]
        if agent_id not in self.registry.sessions:
            self.registry.sessions[agent_id] = {}
        self.registry.sessions[agent_id][service_name] = session

    def get_session(self, agent_id: str, service_name: str):  # -> Optional[Any]
        return self.registry.sessions.get(agent_id, {}).get(service_name)

    # ---- Bulk ----
    def clear_agent(self, agent_id: str) -> None:
        self.registry.sessions.pop(agent_id, None)
        self.registry.tool_cache.pop(agent_id, None)
        self.registry.tool_to_session_map.pop(agent_id, None)
        self.registry.tool_to_service.pop(agent_id, None)
        self.registry.service_states.pop(agent_id, None)
        self.registry.service_metadata.pop(agent_id, None)
        self.registry.service_to_client.pop(agent_id, None)

        client_ids = self.registry.agent_clients.pop(agent_id, [])
        for client_id in client_ids:
            is_used_by_others = any(
                client_id in clients for other_agent, clients in self.registry.agent_clients.items()
                if other_agent != agent_id
            )
            if not is_used_by_others:
                self.registry.client_configs.pop(client_id, None)

    # ---- Optional transaction & health ----
    def begin(self) -> None:
        return

    def commit(self) -> None:
        return

    def rollback(self) -> None:
        return

    def health_check(self) -> bool:
        return True



