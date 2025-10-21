"""
MCPStore Service Management Module
服务管理相关操作的实现
"""

import asyncio
import logging
import time
from typing import Dict, List, Optional, Any, Union, Tuple

from mcpstore.core.models.service import ServiceConnectionState
from .types import ContextType

logger = logging.getLogger(__name__)


class UpdateServiceAuthHelper:
    """更新服务认证助手 - 明确的服务名，避免状态混乱"""

    def __init__(self, context: 'MCPStoreContext', service_name: str, config: Dict[str, Any] = None):
        self._context = context
        self._service_name = service_name  # 🎯 明确的服务名，不会混乱
        self._config = config.copy() if config else {}

    def bearer_auth(self, auth: str) -> 'MCPStoreContext':
        """为指定服务更新 Bearer Token 认证（兼容历史）"""
        # 统一标准化为 Authorization 头
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["Authorization"] = f"Bearer {auth}"
        return self._execute_update()

    def token(self, token: str) -> 'MCPStoreContext':
        """推荐：设置 Bearer Token（等价于 bearer_auth）"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["Authorization"] = f"Bearer {token}"
        return self._execute_update()

    def api_key(self, api_key: str) -> 'MCPStoreContext':
        """推荐：设置 API Key（标准化为 X-API-Key）"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["X-API-Key"] = api_key
        return self._execute_update()

    def custom_headers(self, headers: Dict[str, str]) -> 'MCPStoreContext':
        """为指定服务更新自定义请求头（显式覆盖）"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"].update(headers)
        return self._execute_update()

    def _execute_update(self) -> 'MCPStoreContext':
        """执行更新服务"""
        self._context._sync_helper.run_async(
            self._context.update_service_async(self._service_name, self._config),
            timeout=60.0,
            force_background=True
        )
        return self._context


class ServiceManagementMixin:
    """服务管理混入类"""

    def check_services(self) -> dict:
        """
        健康检查（同步版本），store/agent上下文自动判断
        - store上下文：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - agent上下文：聚合 agent_id 下所有 client_id 的服务健康状态
        """
        return self._sync_helper.run_async(self.check_services_async(), force_background=True)

    async def check_services_async(self) -> dict:
        """
        异步健康检查，store/agent上下文自动判断
        - store上下文：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - agent上下文：聚合 agent_id 下所有 client_id 的服务健康状态
        """
        if self._context_type.name == 'STORE':
            return await self._store.get_health_status()
        elif self._context_type.name == 'AGENT':
            return await self._store.get_health_status(self._agent_id, agent_mode=True)
        else:
            logger.error(f"[check_services] 未知上下文类型: {self._context_type}")
            return {}

    def get_service_info(self, name: str) -> Any:
        """
        获取服务详情（同步版本），支持 store/agent 上下文
        - store上下文：在 global_agent_store 下的所有 client 中查找服务
        - agent上下文：在指定 agent_id 下的所有 client 中查找服务
        """
        return self._sync_helper.run_async(self.get_service_info_async(name), force_background=True)

    async def get_service_info_async(self, name: str) -> Any:
        """
        获取服务详情（异步版本），支持 store/agent 上下文
        - store上下文：在 global_agent_store 下的所有 client 中查找服务
        - agent上下文：在指定 agent_id 下的所有 client 中查找服务（支持本地名称）
        """
        if not name:
            return {}

        if self._context_type == ContextType.STORE:
            logger.debug(f"STORE mode - searching service in global_agent_store: {name}")
            return await self._store.get_service_info(name)
        elif self._context_type == ContextType.AGENT:
            # Agent模式：将名称原样交给 Store 层处理，Store 负责本地名/全局名的鲁棒解析
            logger.debug(f"AGENT mode - searching service in agent({self._agent_id}): {name}")
            return await self._store.get_service_info(name, self._agent_id)
        else:
            logger.error(f"[get_service_info] 未知上下文类型: {self._context_type}")
            return {}

    def update_service(self,
                      name: str,
                      config: Union[Dict[str, Any], None] = None,
                      # 🆕 与用户用法对齐
                      auth: Optional[str] = None,            # 兼容历史：等价于 token
                      token: Optional[str] = None,           # 推荐：Bearer Token
                      api_key: Optional[str] = None,         # 推荐：API Key
                      headers: Optional[Dict[str, str]] = None) -> Union['MCPStoreContext', 'UpdateServiceAuthHelper']:
        """
        更新服务配置，支持安全的链式认证与凭证轮换（合并更新，不会破坏原有关键字段）

        Args:
            name: 服务名称（明确指定，不会混乱）
            config: 新的服务配置（可选，按“补丁”合并语义处理）
            auth/token: Bearer token（两者等价；优先使用 token）
            api_key: API Key（统一标准化为 X-API-Key 头）
            headers: 自定义请求头（显式传入的键优先级最高）

        Returns:
            如果有配置或认证参数：立即执行更新，返回 MCPStoreContext
            如果什么都没有：返回 UpdateServiceAuthHelper 支持链式配置
        """

        if config is not None:
            # 有配置参数：立即执行更新（与认证参数合并，并采用“补丁合并”语义）
            if any([auth, token, api_key, headers]):
                final_config = self._apply_auth_to_update_config(config, auth, token, api_key, headers)
            else:
                final_config = config

            self._sync_helper.run_async(
                self.update_service_async(name, final_config),
                timeout=60.0,
                force_background=True
            )
            return self
        else:
            # 没有配置参数：
            if any([auth, token, api_key, headers]):
                # 纯认证：立即执行（也走补丁合并语义）
                final_config = self._apply_auth_to_update_config({}, auth, token, api_key, headers)
                self._sync_helper.run_async(
                    self.update_service_async(name, final_config),
                    timeout=60.0,
                    force_background=True
                )
                return self
            else:
                # 什么都没有：返回助手用于链式调用
                return UpdateServiceAuthHelper(self, name, {})

    async def update_service_async(self, name: str, config: Dict[str, Any]) -> bool:
        """
        更新服务配置（异步版本）- 合并更新（不会破坏未提供的关键字段）

        Args:
            name: 服务名称
            config: 新的服务配置（作为补丁）

        Returns:
            bool: 更新是否成功
        """
        try:
            #  内部：简单的深度合并（仅对字典执行一层合并；headers 为字典则键级覆盖）
            def _deep_merge(base: Dict[str, Any], patch: Dict[str, Any]) -> Dict[str, Any]:
                result = dict(base or {})
                for k, v in (patch or {}).items():
                    if isinstance(v, dict) and isinstance(result.get(k), dict):
                        merged = dict(result.get(k) or {})
                        merged.update(v)
                        result[k] = merged
                    else:
                        result[k] = v
                return result

            if self._context_type == ContextType.STORE:
                # Store级别：使用原子更新，避免读改写竞态
                from mcpstore.core.configuration.config_write_service import ConfigWriteService
                cws = ConfigWriteService()
                def _mutator(cfg: Dict[str, Any]) -> Dict[str, Any]:
                    servers = dict(cfg.get("mcpServers", {}))
                    if name not in servers:
                        raise KeyError(f"Service {name} not found in store configuration")
                    existing = dict(servers.get(name) or {})
                    merged = _deep_merge(existing, config)
                    servers[name] = merged
                    cfg["mcpServers"] = servers
                    return cfg
                try:
                    success = cws.atomic_update(self._store.config.json_path, _mutator)
                except KeyError as e:
                    logger.error(str(e))
                    return False

                if success:
                    # 触发重新注册
                    if hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                        await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()

                return success
            else:
                # Agent级别：与单一数据源模式对齐——直接更新 mcp.json 并触发同步
                global_name = name
                if self._service_mapper:
                    global_name = self._service_mapper.to_global_name(name)

                from mcpstore.core.configuration.config_write_service import ConfigWriteService
                cws = ConfigWriteService()
                def _mutator(cfg: Dict[str, Any]) -> Dict[str, Any]:
                    servers = dict(cfg.get("mcpServers", {}))
                    if global_name not in servers:
                        raise KeyError(f"Service {global_name} not found in store configuration (agent mode)")
                    existing = dict(servers.get(global_name) or {})
                    merged = _deep_merge(existing, config)
                    servers[global_name] = merged
                    cfg["mcpServers"] = servers
                    return cfg
                try:
                    success = cws.atomic_update(self._store.config.json_path, _mutator)
                except KeyError as e:
                    logger.error(str(e))
                    return False

                if success and hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                    await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()

                # 更新缓存中的 metadata.service_config，确保一致性
                try:
                    # 将元数据更新到全局命名空间，保持与生命周期/工具缓存一致
                    global_agent = self._store.client_manager.global_agent_store_id
                    metadata = self._store.registry.get_service_metadata(global_agent, global_name)
                    if metadata:
                        # 将变更合并到缓存元数据中
                        metadata.service_config = _deep_merge(metadata.service_config or {}, config)
                        self._store.registry.set_service_metadata(global_agent, global_name, metadata)
                except Exception as _:
                    pass

                return success
        except Exception as e:
            logger.error(f"Failed to update service {name}: {e}")
            return False

    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        """
        增量更新服务配置（同步版本）- 推荐使用

        Args:
            name: 服务名称
            updates: 要更新的配置项

        Returns:
            bool: 更新是否成功
        """
        return self._sync_helper.run_async(self.patch_service_async(name, updates), timeout=60.0, force_background=True)

    async def patch_service_async(self, name: str, updates: Dict[str, Any]) -> bool:
        """
        增量更新服务配置（异步版本）- 推荐使用

        Args:
            name: 服务名称
            updates: 要更新的配置项

        Returns:
            bool: 更新是否成功
        """
        try:
            if self._context_type == ContextType.STORE:
                # Store级别：使用原子增量更新
                from mcpstore.core.configuration.config_write_service import ConfigWriteService
                cws = ConfigWriteService()
                def _mutator(cfg: Dict[str, Any]) -> Dict[str, Any]:
                    servers = dict(cfg.get("mcpServers", {}))
                    if name not in servers:
                        raise KeyError(f"Service {name} not found in store configuration")
                    merged = dict(servers[name])
                    merged.update(updates)
                    servers[name] = merged
                    cfg["mcpServers"] = servers
                    return cfg
                try:
                    success = cws.atomic_update(self._store.config.json_path, _mutator)
                except KeyError as e:
                    logger.error(str(e))
                    return False

                if success:
                    # 触发重新注册
                    if hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                        await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()

                return success
            else:
                # Agent级别：与单一数据源模式对齐——直接增量更新 mcp.json 并触发同步
                global_name = name
                if self._service_mapper:
                    global_name = self._service_mapper.to_global_name(name)
                from mcpstore.core.configuration.config_write_service import ConfigWriteService
                cws = ConfigWriteService()
                def _mutator(cfg: Dict[str, Any]) -> Dict[str, Any]:
                    servers = dict(cfg.get("mcpServers", {}))
                    if global_name not in servers:
                        raise KeyError(f"Service {global_name} not found in store configuration (agent mode)")
                    merged = dict(servers[global_name])
                    merged.update(updates)
                    servers[global_name] = merged
                    cfg["mcpServers"] = servers
                    return cfg
                try:
                    success = cws.atomic_update(self._store.config.json_path, _mutator)
                except KeyError as e:
                    logger.error(str(e))
                    return False

                if success and hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                    await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()

                # 更新缓存中的 metadata.service_config，确保一致性
                try:
                    # 将元数据更新到全局命名空间，保持与生命周期/工具缓存一致
                    global_agent = self._store.client_manager.global_agent_store_id
                    metadata = self._store.registry.get_service_metadata(global_agent, global_name)
                    if metadata:
                        metadata.service_config.update(updates)
                        self._store.registry.set_service_metadata(global_agent, global_name, metadata)
                except Exception as _:
                    pass

                return success
        except Exception as e:
            logger.error(f"Failed to patch service {name}: {e}")
            return False

    def delete_service(self, name: str) -> bool:
        """
        删除服务（同步版本）

        Args:
            name: 服务名称

        Returns:
            bool: 删除是否成功
        """
        return self._sync_helper.run_async(self.delete_service_async(name), timeout=60.0, force_background=True)

    async def delete_service_async(self, name: str) -> bool:
        """
        删除服务（异步版本，透明代理）

        Args:
            name: 服务名称（Agent 模式下使用本地名称）

        Returns:
            bool: 删除是否成功
        """
        try:
            if self._context_type == ContextType.STORE:
                # Store级别：删除服务并触发双向同步
                await self._delete_store_service_with_sync(name)
                return True
            else:
                # Agent级别：透明代理删除
                await self._delete_agent_service_with_sync(name)
                return True
        except Exception as e:
            logger.error(f"Failed to delete service {name}: {e}")
            return False

    async def delete_service_two_step(self, service_name: str) -> Dict[str, Any]:
        """
        两步删除服务：从配置文件删除 + 从Registry注销

        Args:
            service_name: 服务名称

        Returns:
            Dict: 包含两步操作结果的字典
        """
        result = {
            "step1_config_removal": False,
            "step2_registry_cleanup": False,
            "step1_error": None,
            "step2_error": None,
            "overall_success": False
        }

        # 第一步：从配置文件删除
        try:
            result["step1_config_removal"] = await self.delete_service_async(service_name)
            if not result["step1_config_removal"]:
                result["step1_error"] = "Failed to remove service from configuration"
        except Exception as e:
            result["step1_error"] = f"Configuration removal failed: {str(e)}"
            logger.error(f"Step 1 (config removal) failed: {e}")

        # 第二步：从Registry清理（即使第一步失败也尝试）
        try:
            if self._context_type == ContextType.STORE:
                # Store级别：清理global_agent_store的Registry
                cleanup_success = await self._store.orchestrator.registry.cleanup_service(service_name)
            else:
                # Agent级别：清理特定agent的Registry
                global_name = service_name
                if self._service_mapper:
                    global_name = self._service_mapper.to_global_name(service_name)
                cleanup_success = await self._store.orchestrator.registry.cleanup_service(global_name, self._agent_id)

            result["step2_registry_cleanup"] = cleanup_success
            if not cleanup_success:
                result["step2_error"] = "Failed to cleanup service from registry"
        except Exception as e:
            result["step2_error"] = f"Registry cleanup failed: {str(e)}"
            logger.warning(f"Step 2 (registry cleanup) failed: {e}")

        result["overall_success"] = result["step1_config_removal"] and result["step2_registry_cleanup"]
        return result

    def reset_config(self, scope: str = "all") -> bool:
        """
        重置配置（同步版本）

        Args:
            scope: 重置范围（仅Store级别有效）
                - "all": 重置所有缓存和所有JSON文件（默认）
                - "global_agent_store": 只重置global_agent_store
        """
        return self._sync_helper.run_async(self.reset_config_async(scope), timeout=60.0, force_background=True)

    async def reset_config_async(self, scope: str = "all") -> bool:
        """
        重置配置（异步版本）- 缓存优先模式

        根据上下文类型执行不同的重置操作：
        - Store上下文：根据scope参数重置不同范围
        - Agent上下文：重置该Agent的所有配置（忽略scope参数）

        Args:
            scope: 重置范围（仅Store级别有效）
                - "all": 重置所有缓存和所有JSON文件（默认）
                - "global_agent_store": 只重置global_agent_store
        """
        try:
            if self._context_type == ContextType.STORE:
                return await self._reset_store_config(scope)
            else:
                return await self._reset_agent_config()
        except Exception as e:
            logger.error(f"Failed to reset config: {e}")
            return False

    async def _reset_store_config(self, scope: str) -> bool:
        """Store级别重置配置的内部实现"""
        try:
            if scope == "all":
                logger.debug("Store level: resetting all caches and JSON files")

                # 1. 清空所有缓存
                self._store.registry.agent_clients.clear()
                self._store.registry.client_configs.clear()

                # 清空其他缓存字段
                self._store.registry.sessions.clear()
                self._store.registry.tool_cache.clear()
                self._store.registry.tool_to_session_map.clear()
                self._store.registry.service_states.clear()
                self._store.registry.service_metadata.clear()
                self._store.registry.service_to_client.clear()

                # 2. 重置mcp.json文件（使用 UnifiedConfigManager 自动刷新缓存）
                default_config = {"mcpServers": {}}
                mcp_success = self._store._unified_config.update_mcp_config(default_config)

                # 3. 单源模式：不再维护分片映射文件
                logger.debug("Single-source mode: skip shard mapping files (agent_clients/client_services)")

                # 4. 触发快照更新（强一致）
                try:
                    gid = self._store.client_manager.global_agent_store_id
                    self._store.registry.tools_changed(gid, aggressive=True)
                except Exception:
                    try:
                        self._store.registry.mark_tools_snapshot_dirty()
                    except Exception:
                        pass

                logger.debug("Store level: all configuration reset completed")
                return mcp_success

            elif scope == "global_agent_store":
                logger.info(" Store级别：只重置global_agent_store")

                # 1. 清空global_agent_store在缓存中的数据
                global_agent_store_id = self._store.client_manager.global_agent_store_id
                self._store.registry.clear(global_agent_store_id)

                # 2. 清空mcp.json文件（使用 UnifiedConfigManager 自动刷新缓存）
                default_config = {"mcpServers": {}}
                mcp_success = self._store._unified_config.update_mcp_config(default_config)

                # 3. 单源模式：不再维护分片映射文件
                logger.debug("Single-source mode: skip shard mapping files (agent_clients/client_services)")

                # 4. 触发快照更新（强一致）
                try:
                    gid = self._store.client_manager.global_agent_store_id
                    self._store.registry.tools_changed(gid, aggressive=True)
                except Exception:
                    try:
                        self._store.registry.mark_tools_snapshot_dirty()
                    except Exception:
                        pass

                logger.info(" Store级别：global_agent_store重置完成")
                return mcp_success

            else:
                logger.error(f"不支持的scope参数: {scope}")
                return False

        except Exception as e:
            logger.error(f"Store级别重置配置失败: {e}")
            return False

    async def _reset_agent_config(self) -> bool:
        """Agent级别重置配置的内部实现"""
        try:
            logger.info(f" Agent级别：重置Agent {self._agent_id} 的所有配置")

            # 1. 清空Agent在缓存中的数据
            self._store.registry.clear(self._agent_id)

            # 2. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f" Agent级别：Agent {self._agent_id} 配置重置完成")
            return True

        except Exception as e:
            logger.error(f"Agent级别重置配置失败: {e}")
            return False

    def show_config(self, scope: str = "all") -> Dict[str, Any]:
        """
        显示配置信息（同步版本）

        Args:
            scope: 显示范围（仅Store级别有效）
                - "all": 显示所有Agent的配置（默认）
                - "global_agent_store": 只显示global_agent_store的配置

        Returns:
            Dict: 配置信息字典
        """
        return self._sync_helper.run_async(self.show_config_async(scope), timeout=60.0, force_background=True)

    async def show_config_async(self, scope: str = "all") -> Dict[str, Any]:
        """
        显示配置信息（异步版本）- 从缓存获取

        根据上下文类型执行不同的显示操作：
        - Store上下文：根据scope参数显示不同范围的配置
        - Agent上下文：显示该Agent的配置（忽略scope参数）

        Args:
            scope: 显示范围（仅Store级别有效）
                - "all": 显示所有Agent的配置（默认）
                - "global_agent_store": 只显示global_agent_store的配置

        Returns:
            Dict: 配置信息字典
        """
        try:
            if self._context_type == ContextType.STORE:
                return await self._show_store_config(scope)
            else:
                return await self._show_agent_config()
        except Exception as e:
            logger.error(f"Failed to show config: {e}")
            return {
                "error": f"Failed to show config: {str(e)}",
                "services": {},
                "summary": {"total_services": 0, "total_clients": 0}
            }

    async def _show_store_config(self, scope: str) -> Dict[str, Any]:
        """Store级别显示配置的内部实现"""
        try:
            if scope == "all":
                logger.info(" Store级别：显示所有Agent的配置")

                # 获取所有Agent ID
                all_agent_ids = self._store.registry.get_all_agent_ids()

                agents_config = {}
                total_services = 0
                total_clients = 0

                for agent_id in all_agent_ids:
                    agent_services = {}
                    agent_client_count = 0

                    # 获取该Agent的所有服务
                    service_names = self._store.registry.get_all_service_names(agent_id)

                    for service_name in service_names:
                        complete_info = self._store.registry.get_complete_service_info(agent_id, service_name)
                        client_id = complete_info.get("client_id")
                        config = complete_info.get("config", {})

                        if client_id:
                            agent_services[service_name] = {
                                "client_id": client_id,
                                "config": config
                            }
                            agent_client_count += 1

                    if agent_services:  # 只包含有服务的Agent
                        agents_config[agent_id] = {
                            "services": agent_services
                        }
                        total_services += len(agent_services)
                        total_clients += agent_client_count

                return {
                    "agents": agents_config,
                    "summary": {
                        "total_agents": len(agents_config),
                        "total_services": total_services,
                        "total_clients": total_clients
                    }
                }

            elif scope == "global_agent_store":
                logger.info(" Store级别：只显示global_agent_store的配置")

                global_agent_store_id = self._store.client_manager.global_agent_store_id
                return await self._get_single_agent_config(global_agent_store_id)

            else:
                logger.error(f"不支持的scope参数: {scope}")
                return {
                    "error": f"Unsupported scope parameter: {scope}",
                    "services": {},
                    "summary": {"total_services": 0, "total_clients": 0}
                }

        except Exception as e:
            logger.error(f"Store级别显示配置失败: {e}")
            return {
                "error": f"Failed to show store config: {str(e)}",
                "services": {},
                "summary": {"total_services": 0, "total_clients": 0}
            }

    async def _show_agent_config(self) -> Dict[str, Any]:
        """Agent级别显示配置的内部实现"""
        try:
            logger.info(f" Agent级别：显示Agent {self._agent_id} 的配置")

            # 检查Agent是否存在
            all_agent_ids = self._store.registry.get_all_agent_ids()
            if self._agent_id not in all_agent_ids:
                logger.warning(f"Agent {self._agent_id} not found")
                return {
                    "error": f"Agent '{self._agent_id}' not found",
                    "agent_id": self._agent_id,
                    "services": {},
                    "summary": {"total_services": 0, "total_clients": 0}
                }

            return await self._get_single_agent_config(self._agent_id)

        except Exception as e:
            logger.error(f"Agent级别显示配置失败: {e}")
            return {
                "error": f"Failed to show agent config: {str(e)}",
                "agent_id": self._agent_id,
                "services": {},
                "summary": {"total_services": 0, "total_clients": 0}
            }

    async def _get_single_agent_config(self, agent_id: str) -> Dict[str, Any]:
        """获取单个Agent的配置信息"""
        try:
            services_config = {}
            client_count = 0

            # 获取该Agent的所有服务
            service_names = self._store.registry.get_all_service_names(agent_id)

            for service_name in service_names:
                complete_info = self._store.registry.get_complete_service_info(agent_id, service_name)
                client_id = complete_info.get("client_id")
                config = complete_info.get("config", {})

                if client_id:
                    # Agent级别显示实际的服务名（带后缀的版本）
                    services_config[service_name] = {
                        "client_id": client_id,
                        "config": config
                    }
                    client_count += 1

            return {
                "agent_id": agent_id,
                "services": services_config,
                "summary": {
                    "total_services": len(services_config),
                    "total_clients": client_count
                }
            }

        except Exception as e:
            logger.error(f"获取Agent {agent_id} 配置失败: {e}")
            return {
                "error": f"Failed to get config for agent '{agent_id}': {str(e)}",
                "agent_id": agent_id,
                "services": {},
                "summary": {"total_services": 0, "total_clients": 0}
            }

    def delete_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """
        删除服务配置（同步版本）

        Args:
            client_id_or_service_name: client_id或服务名

        Returns:
            Dict: 删除结果
        """
        return self._sync_helper.run_async(self.delete_config_async(client_id_or_service_name), timeout=60.0, force_background=True)

    async def delete_config_async(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """
        删除服务配置（异步版本）

        支持智能参数识别：
        - 如果传入client_id，直接使用
        - 如果传入服务名，自动查找对应的client_id
        - Agent级别严格隔离，只在指定agent范围内查找

        Args:
            client_id_or_service_name: client_id或服务名

        Returns:
            Dict: 删除结果
        """
        try:
            if self._context_type == ContextType.STORE:
                return await self._delete_store_config(client_id_or_service_name)
            else:
                return await self._delete_agent_config(client_id_or_service_name)
        except Exception as e:
            logger.error(f"Failed to delete config: {e}")
            return {
                "success": False,
                "error": f"Failed to delete config: {str(e)}",
                "client_id": None,
                "service_name": None
            }

    def update_config(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        更新服务配置（同步版本）

        Args:
            client_id_or_service_name: client_id或服务名
            new_config: 新的配置信息

        Returns:
            Dict: 更新结果
        """
        return self._sync_helper.run_async(self.update_config_async(client_id_or_service_name, new_config), timeout=60.0, force_background=True)

    async def update_config_async(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        更新服务配置（异步版本）

        支持智能参数识别和多种配置格式：
        - 参数识别：client_id或服务名自动识别
        - 配置格式：支持简化格式和mcpServers格式
        - 字段验证：不允许修改服务名，不允许新增字段类型
        - Agent级别严格隔离

        Args:
            client_id_or_service_name: client_id或服务名
            new_config: 新的配置信息

        Returns:
            Dict: 更新结果
        """
        try:
            if self._context_type == ContextType.STORE:
                return await self._update_store_config(client_id_or_service_name, new_config)
            else:
                return await self._update_agent_config(client_id_or_service_name, new_config)
        except Exception as e:
            logger.error(f"Failed to update config: {e}")
            return {
                "success": False,
                "error": f"Failed to update config: {str(e)}",
                "client_id": None,
                "service_name": None,
                "old_config": None,
                "new_config": None
            }

    def _is_deterministic_client_id(self, identifier: str) -> bool:
        """使用 ClientIDGenerator 统一判断确定性client_id格式"""
        try:
            from mcpstore.core.utils.id_generator import ClientIDGenerator
            return ClientIDGenerator.is_deterministic_format(identifier)
        except Exception:
            return False

    def _parse_deterministic_client_id(self, client_id: str, agent_id: str) -> Tuple[str, str]:
        """使用 ClientIDGenerator 统一解析确定性client_id，并验证agent范围"""
        from mcpstore.core.utils.id_generator import ClientIDGenerator
        parsed = ClientIDGenerator.parse_client_id(client_id)
        if parsed.get("type") == "store":
            global_agent_store_id = self._store.client_manager.global_agent_store_id
            if agent_id != global_agent_store_id:
                raise ValueError(f"Store client_id '{client_id}' cannot be used with agent '{agent_id}'")
            return client_id, parsed.get("service_name")
        elif parsed.get("type") == "agent":
            if parsed.get("agent_id") != agent_id:
                raise ValueError(f"Client_id '{client_id}' belongs to agent '{parsed.get('agent_id')}', not '{agent_id}'")
            return client_id, parsed.get("service_name")
        raise ValueError(f"Cannot parse client_id format: {client_id}")

    def _validate_resolved_mapping(self, client_id: str, service_name: str, agent_id: str) -> bool:
        """
        验证解析后的client_id和service_name映射是否有效

        Args:
            client_id: 解析出的client_id
            service_name: 解析出的service_name
            agent_id: Agent ID

        Returns:
            bool: 映射是否有效
        """
        try:
            # 检查client_id是否存在于agent的映射中
            agent_clients = self._store.registry.get_agent_clients_from_cache(agent_id)
            if client_id not in agent_clients:
                logger.debug(f" [VALIDATE_MAPPING] client_id '{client_id}' not found in agent '{agent_id}' clients")
                return False

            # 检查service_name是否存在于Registry中
            existing_client_id = self._store.registry.get_service_client_id(agent_id, service_name)
            if existing_client_id != client_id:
                logger.debug(f" [VALIDATE_MAPPING] service '{service_name}' maps to different client_id: expected={client_id}, actual={existing_client_id}")
                return False

            return True
        except Exception as e:
            logger.debug(f" [VALIDATE_MAPPING] 验证失败: {e}")
            return False

    def _resolve_client_id(self, client_id_or_service_name: str, agent_id: str) -> Tuple[str, str]:
        """
        智能解析client_id或服务名（使用最新的确定性算法）

        Args:
            client_id_or_service_name: 用户输入的参数
            agent_id: Agent ID（用于范围限制）

        Returns:
            Tuple[client_id, service_name]: 解析后的client_id和服务名

        Raises:
            ValueError: 当参数无法解析或不存在时
        """
        logger.debug(f"[RESOLVE_CLIENT_ID] start value='{client_id_or_service_name}' agent='{agent_id}'")

        from .agent_service_mapper import AgentServiceMapper
        global_agent_id = self._store.client_manager.global_agent_store_id

        # 1) 优先：确定性 client_id 直接解析
        if self._is_deterministic_client_id(client_id_or_service_name):
            try:
                client_id, service_name = self._parse_deterministic_client_id(client_id_or_service_name, agent_id)
                logger.debug(f"[RESOLVE_CLIENT_ID] deterministic_ok client_id={client_id} service_name={service_name}")
                return client_id, service_name
            except ValueError as e:
                logger.debug(f"[RESOLVE_CLIENT_ID] deterministic_parse_failed error={e}")
                # 继续按服务名处理

        # 2) Agent 模式：透明代理到 Store（不依赖 Agent 命名空间缓存）
        if self._context_type == ContextType.AGENT and agent_id != global_agent_id:
            # 2.1 判断输入是本地名还是全局名
            input_name = client_id_or_service_name
            global_service_name = None

            if AgentServiceMapper.is_any_agent_service(input_name):
                # 输入是全局名，校验归属
                try:
                    parsed_agent_id, local_name = AgentServiceMapper.parse_agent_service_name(input_name)
                    if parsed_agent_id != agent_id:
                        raise ValueError(f"Service '{input_name}' belongs to agent '{parsed_agent_id}', not '{agent_id}'")
                    global_service_name = input_name
                except ValueError as e:
                    raise ValueError(f"Invalid agent service name '{input_name}': {e}")
            else:
                # 输入是本地名：优先用映射，其次用规则推导
                mapped = self._store.registry.get_global_name_from_agent_service(agent_id, input_name)
                global_service_name = mapped or AgentServiceMapper(agent_id).to_global_name(input_name)

            # 2.2 在 Store 命名空间解析 client_id
            client_id = self._store.registry.get_service_client_id(global_agent_id, global_service_name)
            if not client_id:
                available = ', '.join(self._store.registry.get_all_service_names(global_agent_id)) or 'None'
                raise ValueError(
                    f"Service '{input_name}' (global '{global_service_name}') not found in store. Available services: {available}"
                )

            logger.debug(f"[RESOLVE_CLIENT_ID] agent_proxy_ok local_or_global='{input_name}' -> global='{global_service_name}' client_id={client_id}")
            return client_id, global_service_name

        # 3) Store 模式：直接在 Store 命名空间解析
        service_name = client_id_or_service_name
        service_names = self._store.registry.get_all_service_names(agent_id)
        if service_name in service_names:
            client_id = self._store.registry.get_service_client_id(agent_id, service_name)
            if client_id:
                logger.debug(f"[RESOLVE_CLIENT_ID] store_lookup_ok service={service_name} client_id={client_id}")
                return client_id, service_name
            else:
                raise ValueError(f"Service '{service_name}' found but no client_id mapping")

        available_services = ', '.join(service_names) if service_names else 'None'
        raise ValueError(f"Service '{service_name}' not found in store. Available services: {available_services}")

    async def _delete_store_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """Store级别删除配置的内部实现"""
        try:
            logger.info(f"🗑️ Store级别：删除配置 {client_id_or_service_name}")

            global_agent_store_id = self._store.client_manager.global_agent_store_id

            # 解析client_id和服务名
            client_id, service_name = self._resolve_client_id(client_id_or_service_name, global_agent_store_id)

            logger.info(f"🗑️ 解析结果: client_id={client_id}, service_name={service_name}")

            # 验证服务存在
            if not self._store.registry.get_session(global_agent_store_id, service_name):
                logger.warning(f"Service {service_name} not found in registry, but continuing with cleanup")

            # 事务性删除：先删除文件配置，再删除缓存
            # 1. 从mcp.json中删除服务配置（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.remove_service_config(service_name)
            if success:
                logger.info(f"🗑️ 已从mcp.json删除服务: {service_name}，缓存已同步")

            # 2. 从缓存中删除服务（包括工具和会话）
            self._store.registry.remove_service(global_agent_store_id, service_name)

            # 3. 删除Service-Client映射
            self._store.registry.remove_service_client_mapping(global_agent_store_id, service_name)

            # 4. 删除Client配置
            self._store.registry.remove_client_config(client_id)

            # 5. 删除Agent-Client映射
            self._store.registry.remove_agent_client_mapping(global_agent_store_id, client_id)

            # 6. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f" Store级别：配置删除完成 {service_name}")

            # 触发快照更新（强一致）
            try:
                self._store.registry.tools_changed(global_agent_store_id, aggressive=True)
            except Exception:
                try:
                    self._store.registry.mark_tools_snapshot_dirty()
                except Exception:
                    pass

            return {
                "success": True,
                "message": f"Service '{service_name}' deleted successfully",
                "client_id": client_id,
                "service_name": service_name
            }

        except Exception as e:
            logger.error(f"Store级别删除配置失败: {e}")
            return {
                "success": False,
                "error": f"Failed to delete store config: {str(e)}",
                "client_id": None,
                "service_name": None
            }

    async def _delete_agent_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """Agent级别删除配置的内部实现"""
        try:
            logger.info(f"🗑️ Agent级别：删除Agent {self._agent_id} 的配置 {client_id_or_service_name}")

            # 解析client_id和服务名
            client_id, service_name = self._resolve_client_id(client_id_or_service_name, self._agent_id)

            logger.info(f"🗑️ 解析结果: client_id={client_id}, service_name={service_name}")

            # 验证服务存在
            if not self._store.registry.get_session(self._agent_id, service_name):
                logger.warning(f"Service {service_name} not found in registry for agent {self._agent_id}, but continuing with cleanup")

            # Agent级别删除：只删除缓存，不修改mcp.json
            # 1. 从缓存中删除服务（包括工具和会话）
            self._store.registry.remove_service(self._agent_id, service_name)

            # 2. 删除Service-Client映射
            self._store.registry.remove_service_client_mapping(self._agent_id, service_name)

            # 3. 删除Client配置
            self._store.registry.remove_client_config(client_id)

            # 4. 删除Agent-Client映射
            self._store.registry.remove_agent_client_mapping(self._agent_id, client_id)

            # 5. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f" Agent级别：配置删除完成 {service_name}")

            # 触发快照更新（强一致）
            try:
                gid = self._store.client_manager.global_agent_store_id
                self._store.registry.tools_changed(gid, aggressive=True)
            except Exception:
                try:
                    self._store.registry.mark_tools_snapshot_dirty()
                except Exception:
                    pass

            return {
                "success": True,
                "message": f"Service '{service_name}' deleted successfully from agent '{self._agent_id}'",
                "client_id": client_id,
                "service_name": service_name
            }

        except Exception as e:
            logger.error(f"Agent级别删除配置失败: {e}")
            return {
                "success": False,
                "error": f"Failed to delete agent config: {str(e)}",
                "client_id": None,
                "service_name": None
            }

    def _validate_and_normalize_config(self, new_config: Dict[str, Any], service_name: str, old_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        验证和标准化配置

        Args:
            new_config: 新配置
            service_name: 服务名
            old_config: 原配置

        Returns:
            Dict: 标准化后的配置

        Raises:
            ValueError: 配置验证失败
        """
        # 1. 处理配置格式
        if "mcpServers" in new_config:
            # mcpServers格式
            if len(new_config["mcpServers"]) != 1:
                raise ValueError("mcpServers format must contain exactly one service")

            config_service_name = list(new_config["mcpServers"].keys())[0]
            if config_service_name != service_name:
                raise ValueError(f"Cannot change service name from '{service_name}' to '{config_service_name}'")

            normalized_config = new_config["mcpServers"][service_name]
        else:
            # 简化格式
            if "name" in new_config:
                raise ValueError("Cannot modify service name in config update")
            normalized_config = new_config.copy()

        # 2. 验证字段类型一致性
        old_config_keys = set(old_config.keys())
        new_config_keys = set(normalized_config.keys())

        # 检查是否有新增的字段类型
        new_fields = new_config_keys - old_config_keys
        if new_fields:
            raise ValueError(f"Cannot add new field types: {list(new_fields)}. Only existing fields can be updated.")

        # 3. 验证字段值的合理性
        for key, value in normalized_config.items():
            if key in old_config:
                old_type = type(old_config[key])
                new_type = type(value)

                # 允许的类型转换
                if old_type != new_type:
                    # 允许字符串和数字之间的转换
                    if not ((old_type in [str, int, float] and new_type in [str, int, float]) or
                            (old_type == list and new_type == list)):
                        raise ValueError(f"Field '{key}' type mismatch: expected {old_type.__name__}, got {new_type.__name__}")

        return normalized_config

    async def _update_store_config(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        """Store级别更新配置的内部实现"""
        try:
            logger.info(f" Store级别：更新配置 {client_id_or_service_name}")

            global_agent_store_id = self._store.client_manager.global_agent_store_id

            # 解析client_id和服务名
            client_id, service_name = self._resolve_client_id(client_id_or_service_name, global_agent_store_id)

            logger.info(f" 解析结果: client_id={client_id}, service_name={service_name}")

            # 获取当前配置
            old_complete_info = self._store.registry.get_complete_service_info(global_agent_store_id, service_name)
            old_config = old_complete_info.get("config", {})

            if not old_config:
                raise ValueError(f"Service '{service_name}' configuration not found")

            # 验证和标准化新配置
            normalized_config = self._validate_and_normalize_config(new_config, service_name, old_config)

            logger.info(f" 配置验证通过，开始更新: {service_name}")

            # 1. 清空服务的工具和会话数据
            self._store.registry.clear_service_tools_only(global_agent_store_id, service_name)

            # 2. 更新Client配置缓存
            self._store.registry.update_client_config(client_id, {
                "mcpServers": {service_name: normalized_config}
            })

            # 3. 设置服务状态为INITIALIZING并更新元数据
            from mcpstore.core.models.service import ServiceConnectionState
            self._store.registry.set_service_state(global_agent_store_id, service_name, ServiceConnectionState.INITIALIZING)

            # 更新服务元数据中的配置
            metadata = self._store.registry.get_service_metadata(global_agent_store_id, service_name)
            if metadata:
                metadata.service_config = normalized_config
                metadata.consecutive_failures = 0
                metadata.error_message = None
                from datetime import datetime
                metadata.state_entered_time = datetime.now()
                self._store.registry.set_service_metadata(global_agent_store_id, service_name, metadata)

            # 4. 更新mcp.json文件（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.add_service_config(service_name, normalized_config)
            if not success:
                raise Exception(f"Failed to update service config for {service_name}")

            # 5. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            # 6. 触发生命周期管理器重新初始化服务
            self._store.orchestrator.lifecycle_manager.initialize_service(
                global_agent_store_id, service_name, normalized_config
            )

            logger.info(f" Store级别：配置更新完成 {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' configuration updated successfully",
                "client_id": client_id,
                "service_name": service_name,
                "old_config": old_config,
                "new_config": normalized_config
            }

        except Exception as e:
            logger.error(f"Store级别更新配置失败: {e}")
            return {
                "success": False,
                "error": f"Failed to update store config: {str(e)}",
                "client_id": None,
                "service_name": None,
                "old_config": None,
                "new_config": None
            }

    async def _update_agent_config(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        """Agent级别更新配置的内部实现"""
        try:
            logger.info(f" Agent级别：更新Agent {self._agent_id} 的配置 {client_id_or_service_name}")

            # 解析client_id和服务名
            client_id, service_name = self._resolve_client_id(client_id_or_service_name, self._agent_id)

            logger.info(f" 解析结果: client_id={client_id}, service_name={service_name}")

            # 获取当前配置
            old_complete_info = self._store.registry.get_complete_service_info(self._agent_id, service_name)
            old_config = old_complete_info.get("config", {})

            if not old_config:
                raise ValueError(f"Service '{service_name}' configuration not found")

            # 验证和标准化新配置
            normalized_config = self._validate_and_normalize_config(new_config, service_name, old_config)

            logger.info(f" 配置验证通过，开始更新: {service_name}")

            # 1. 清空服务的工具和会话数据
            self._store.registry.clear_service_tools_only(self._agent_id, service_name)

            # 2. 更新Client配置缓存
            self._store.registry.update_client_config(client_id, {
                "mcpServers": {service_name: normalized_config}
            })

            # 3. 设置服务状态为INITIALIZING并更新元数据
            from mcpstore.core.models.service import ServiceConnectionState
            self._store.registry.set_service_state(self._agent_id, service_name, ServiceConnectionState.INITIALIZING)

            # 更新服务元数据中的配置
            metadata = self._store.registry.get_service_metadata(self._agent_id, service_name)
            if metadata:
                metadata.service_config = normalized_config
                metadata.consecutive_failures = 0
                metadata.error_message = None
                from datetime import datetime
                metadata.state_entered_time = datetime.now()
                self._store.registry.set_service_metadata(self._agent_id, service_name, metadata)

            # 4. 单源模式：不再同步到分片文件（Agent级别不更新mcp.json）
            logger.info("Single-source mode: skip shard mapping files sync")

            # 5. 触发生命周期管理器重新初始化服务
            self._store.orchestrator.lifecycle_manager.initialize_service(
                self._agent_id, service_name, normalized_config
            )

            logger.info(f" Agent级别：配置更新完成 {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' configuration updated successfully for agent '{self._agent_id}'",
                "client_id": client_id,
                "service_name": service_name,
                "old_config": old_config,
                "new_config": normalized_config
            }

        except Exception as e:
            logger.error(f"Agent级别更新配置失败: {e}")
            return {
                "success": False,
                "error": f"Failed to update agent config: {str(e)}",
                "client_id": None,
                "service_name": None,
                "old_config": None,
                "new_config": None
            }

    def get_service_status(self, name: str) -> dict:
        """获取单个服务的状态信息（同步版本）"""
        return self._sync_helper.run_async(self.get_service_status_async(name), force_background=True)

    async def get_service_status_async(self, name: str) -> dict:
        """获取单个服务的状态信息"""
        try:
            if self._context_type == ContextType.STORE:
                return self._store.orchestrator.get_service_status(name)
            else:
                # Agent模式：转换服务名称
                global_name = name
                if self._service_mapper:
                    global_name = self._service_mapper.to_global_name(name)
                # 透明代理：在全局命名空间查询状态
                return self._store.orchestrator.get_service_status(global_name)
        except Exception as e:
            logger.error(f"Failed to get service status for {name}: {e}")
            return {"status": "error", "error": str(e)}

    def restart_service(self, name: str) -> bool:
        """重启指定服务（同步版本）"""
        # 使用持久后台事件循环，避免 asyncio.run 的临时事件循环导致事件处理器被取消
        return self._sync_helper.run_async(self.restart_service_async(name), force_background=True)

    async def restart_service_async(self, name: str) -> bool:
        """重启指定服务（透明代理）"""
        try:
            if self._context_type == ContextType.STORE:
                return await self._store.orchestrator.restart_service(name)
            else:
                # Agent模式：透明代理 - 将本地服务名映射到全局服务名，并在全局命名空间执行重启
                global_name = await self._map_agent_service_to_global(name)
                global_agent = self._store.client_manager.global_agent_store_id
                return await self._store.orchestrator.restart_service(global_name, global_agent)
        except Exception as e:
            logger.error(f"Failed to restart service {name}: {e}")
            return False

    # === Lifecycle-only disconnection (no config/registry deletion) ===
    def disconnect_service(self, name: str, reason: str = "user_requested") -> bool:
        """
        断开服务（同步版本）- 仅生命周期断链：
        - 不修改 mcp.json
        - 不从注册表删除服务
        - 将状态置为 disconnected，并清空工具展示
        """
        return self._sync_helper.run_async(
            self.disconnect_service_async(name, reason=reason),
            timeout=60.0,
            force_background=True
        )

    async def disconnect_service_async(self, name: str, reason: str = "user_requested") -> bool:
        """
        断开服务（异步版本）- 仅生命周期断链：不改配置/不删注册表。

        Store 上下文：name 视为全局名；
        Agent 上下文：自动将本地名映射为全局名后断开。
        """
        try:
            global_agent_id = self._store.client_manager.global_agent_store_id
            if self._context_type == ContextType.STORE:
                global_name = name
            else:
                global_name = await self._map_agent_service_to_global(name)

            # 调用生命周期管理器执行优雅断开
            lm = self._store.orchestrator.lifecycle_manager
            await lm.graceful_disconnect(global_agent_id, global_name, reason)

            # 清空工具展示缓存（仅清工具，不删除服务实体）
            try:
                self._store.registry.clear_service_tools_only(global_agent_id, global_name)
            except Exception:
                pass
            # 触发快照更新（强一致）
            try:
                self._store.registry.tools_changed(global_agent_id, aggressive=True)
            except Exception:
                try:
                    self._store.registry.mark_tools_snapshot_dirty()
                except Exception:
                    pass

            return True
        except Exception as e:
            logger.error(f"[DISCONNECT_SERVICE] Failed to disconnect '{name}': {e}")
            return False

    # ===  新增：Agent 透明代理辅助方法 ===

    async def _map_agent_service_to_global(self, local_name: str) -> str:
        """
        将 Agent 的本地服务名映射到全局服务名

        Args:
            local_name: Agent 中的本地服务名

        Returns:
            str: 全局服务名
        """
        try:
            if self._agent_id:
                # 尝试从映射关系中获取全局名称
                global_name = self._store.registry.get_global_name_from_agent_service(self._agent_id, local_name)
                if global_name:
                    logger.debug(f" [SERVICE_PROXY] 服务名映射: {local_name} → {global_name}")
                    return global_name

            # 如果映射失败，可能是 Store 原生服务，直接返回
            logger.debug(f" [SERVICE_PROXY] 无映射，使用原名: {local_name}")
            return local_name

        except Exception as e:
            logger.error(f" [SERVICE_PROXY] 服务名映射失败: {e}")
            return local_name

    async def _delete_store_service_with_sync(self, service_name: str):
        """Store 服务删除（带双向同步）"""
        try:
            # 1. 从 Registry 中删除
            self._store.registry.remove_service(
                self._store.client_manager.global_agent_store_id,
                service_name
            )

            # 2. 从 mcp.json 中删除（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.remove_service_config(service_name)
            
            if success:
                logger.info(f" [SERVICE_DELETE] Store 服务删除成功: {service_name}，缓存已同步")
            else:
                logger.error(f" [SERVICE_DELETE] Store 服务删除失败: {service_name}")

            # 3. 触发双向同步（如果是 Agent 服务）
            if hasattr(self._store, 'bidirectional_sync_manager'):
                await self._store.bidirectional_sync_manager.handle_service_deletion_with_sync(
                    self._store.client_manager.global_agent_store_id,
                    service_name
                )

        except Exception as e:
            logger.error(f" [SERVICE_DELETE] Store 服务删除失败 {service_name}: {e}")
            raise

    async def _delete_agent_service_with_sync(self, local_name: str):
        """Agent 服务删除（带双向同步）"""
        try:
            # 1. 获取全局名称
            global_name = self._store.registry.get_global_name_from_agent_service(self._agent_id, local_name)
            if not global_name:
                logger.warning(f" [SERVICE_DELETE] 未找到映射关系: {self._agent_id}:{local_name}")
                return

            # 2. 从 Agent 缓存中删除
            self._store.registry.remove_service(self._agent_id, local_name)

            # 3. 从 Store 缓存中删除
            self._store.registry.remove_service(
                self._store.client_manager.global_agent_store_id,
                global_name
            )

            # 4. 移除映射关系
            self._store.registry.remove_agent_service_mapping(self._agent_id, local_name)

            # 5. 从 mcp.json 中删除（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.remove_service_config(global_name)
            
            if success:
                logger.info(f" [SERVICE_DELETE] Agent 服务删除成功: {local_name} → {global_name}，缓存已同步")
            else:
                logger.error(f" [SERVICE_DELETE] Agent 服务删除失败: {local_name} → {global_name}")

            # 6. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

        except Exception as e:
            logger.error(f" [SERVICE_DELETE] Agent 服务删除失败 {self._agent_id}:{local_name}: {e}")
            raise

    def show_mcpconfig(self) -> Dict[str, Any]:
        """
        根据当前上下文（store/agent）获取对应的配置信息

        Returns:
            Dict[str, Any]: Store上下文返回MCP JSON格式，Agent上下文返回client配置字典
        """
        if self._context_type == ContextType.STORE:
            # Store上下文：返回MCP JSON格式的配置（从缓存读取，更高效）
            try:
                config = self._store._unified_config.get_mcp_config()
                # 确保返回格式正确
                if isinstance(config, dict) and 'mcpServers' in config:
                    return config
                else:
                    logger.warning("Invalid MCP config format")
                    return {"mcpServers": {}}
            except Exception as e:
                logger.error(f"Failed to show MCP config: {e}")
                return {"mcpServers": {}}
        else:
            # Agent上下文：返回所有相关client配置的字典
            agent_id = self._agent_id
            client_ids = self._store.registry.get_agent_clients_from_cache(agent_id)

            # 获取每个client的配置
            result = {}
            for client_id in client_ids:
                client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
                if client_config:
                    result[client_id] = client_config

            return result

    def wait_service(self, client_id_or_service_name: str,
                    status: Union[str, List[str]] = 'healthy',
                    timeout: float = 10.0,
                    raise_on_timeout: bool = False) -> bool:
        """
        等待服务达到指定状态（同步版本）

        Args:
            client_id_or_service_name: client_id或服务名（智能识别）
            status: 目标状态，可以是单个状态字符串或状态列表
            timeout: 超时时间（秒），默认10秒
            raise_on_timeout: 超时时是否抛出异常，默认False

        Returns:
            bool: 成功达到目标状态返回True，超时返回False

        Raises:
            TimeoutError: 当raise_on_timeout=True且超时时抛出
            ValueError: 当参数无法解析时抛出
        """
        return self._sync_helper.run_async(
            self.wait_service_async(client_id_or_service_name, status, timeout, raise_on_timeout),
            timeout=timeout + 1.0,  # 给异步版本额外1秒缓冲
            force_background=True
        )

    async def wait_service_async(self, client_id_or_service_name: str,
                               status: Union[str, List[str]] = 'healthy',
                               timeout: float = 10.0,
                               raise_on_timeout: bool = False) -> bool:
        """
        等待服务达到指定状态（异步版本）

        Args:
            client_id_or_service_name: client_id或服务名（智能识别）
            status: 目标状态，可以是单个状态字符串或状态列表
            timeout: 超时时间（秒），默认10秒
            raise_on_timeout: 超时时是否抛出异常，默认False

        Returns:
            bool: 成功达到目标状态返回True，超时返回False

        Raises:
            TimeoutError: 当raise_on_timeout=True且超时时抛出
            ValueError: 当参数无法解析时抛出
        """
        try:
            # 解析参数
            agent_scope = self._agent_id if self._context_type == ContextType.AGENT else self._store.client_manager.global_agent_store_id
            client_id, service_name = self._resolve_client_id(client_id_or_service_name, agent_scope)

            # 在纯视图模式下，Agent 的状态查询统一使用全局命名空间
            status_agent_key = self._store.client_manager.global_agent_store_id


            # 诊断：解析后的作用域与标识
            try:
                logger.info(f"[WAIT_SERVICE] resolved agent_scope={agent_scope} client_id='{client_id}' service='{service_name}' status_agent_key={status_agent_key}")
            except Exception:
                pass

            # 解析等待模式
            change_mode = False
            if isinstance(status, str) and status.lower() == 'change':
                change_mode = True
                logger.info(f"[WAIT_SERVICE] start mode=change service='{service_name}' timeout={timeout}s")
                try:
                    initial_status = (self._store.orchestrator.get_service_status(service_name, status_agent_key) or {}).get("status", "unknown")
                except Exception as _e_init:
                    logger.debug(f"[WAIT_SERVICE] initial_status_error service='{service_name}' error={_e_init}")
                    initial_status = "unknown"
            else:
                # 规范化目标状态
                target_statuses = self._normalize_target_statuses(status)
                logger.info(f"[WAIT_SERVICE] start mode=target service='{service_name}' client_id='{client_id}' target={target_statuses} timeout={timeout}s")

            start_time = time.time()
            poll_interval = 0.2  # 200ms轮询间隔
            prev_status = None
            last_log = start_time

            while True:
                # 检查超时
                elapsed = time.time() - start_time
                if elapsed >= timeout:
                    if change_mode:
                        msg = f"[WAIT_SERVICE] timeout mode=change service='{service_name}' from='{initial_status}' elapsed={elapsed:.2f}s"
                    else:
                        msg = f"[WAIT_SERVICE] timeout mode=target service='{service_name}' target={target_statuses} last='{prev_status}' elapsed={elapsed:.2f}s"
                    logger.warning(msg)
                    if raise_on_timeout:
                        raise TimeoutError(msg)
                    return False

                # 获取当前状态（先读一次缓存，随后在必要时读一次新缓存以防止竞态）
                try:

                    status_dict = self._store.orchestrator.get_service_status(service_name, status_agent_key) or {}
                    current_status = status_dict.get("status", "unknown")

                    # 仅在状态变化或每2秒节流一次打印
                    now = time.time()
                    if current_status != prev_status or (now - last_log) > 2.0:
                        logger.debug(f"[WAIT_SERVICE] status service='{service_name}' value='{current_status}'")
                        # 对比 orchestrator 与 registry 的状态及最近健康检查（节流打印）
                        try:
                            reg_state = self._store.registry.get_service_state(status_agent_key, service_name)
                            meta = self._store.registry.get_service_metadata(status_agent_key, service_name)
                            last_check_ts = meta.last_health_check.isoformat() if getattr(meta, 'last_health_check', None) else None
                            logger.debug(f"[WAIT_SERVICE] compare orchestrator='{current_status}' registry='{getattr(reg_state,'value',reg_state)}' last_check={last_check_ts}")
                        except Exception:
                            pass

                        prev_status, last_log = current_status, now

                    if change_mode:
                        if current_status != initial_status:
                            logger.info(f"[WAIT_SERVICE] done mode=change service='{service_name}' from='{initial_status}' to='{current_status}' elapsed={elapsed:.2f}s")
                            return True
                    else:
                        # 检查是否达到目标状态
                        if current_status in target_statuses:
                            logger.info(f"[WAIT_SERVICE] done mode=target service='{service_name}' reached='{current_status}' elapsed={elapsed:.2f}s")
                            return True
                except Exception as e:
                    # 降级到 debug，避免无意义刷屏
                    logger.debug(f"[WAIT_SERVICE] status_error service='{service_name}' error={e}")
                    # 继续轮询

                # 等待下次轮询
                await asyncio.sleep(poll_interval)

        except ValueError as e:
            logger.error(f"[WAIT_SERVICE] param_error error={e}")
            raise
        except Exception as e:
            logger.error(f"[WAIT_SERVICE] unexpected_error error={e}")
            if raise_on_timeout:
                raise
            return False

    def _normalize_target_statuses(self, status: Union[str, List[str]]) -> List[str]:
        """
        规范化目标状态参数

        Args:
            status: 状态参数，可以是字符串或列表

        Returns:
            List[str]: 规范化的状态列表

        Raises:
            ValueError: 当状态值无效时抛出
        """
        # 获取有效的状态值
        valid_statuses = {state.value for state in ServiceConnectionState}

        if isinstance(status, str):
            target_statuses = [status]
        elif isinstance(status, list):
            target_statuses = status
        else:
            raise ValueError(f"Status must be string or list, got {type(status)}")

        # 验证状态值
        for s in target_statuses:
            if s not in valid_statuses:
                raise ValueError(f"Invalid status '{s}'. Valid statuses are: {sorted(valid_statuses)}")

        return target_statuses

    def _apply_auth_to_update_config(self, config: Dict[str, Any],
                                    auth: Optional[str],
                                    token: Optional[str],
                                    api_key: Optional[str],
                                    headers: Optional[Dict[str, str]]) -> Dict[str, Any]:
        """将认证配置应用到更新配置中（标准化为 headers + 合并语义）"""
        final_config = config.copy() if config else {}

        # 构造标准化后的 headers
        normalized_headers: Dict[str, str] = {}
        eff_token = token if token else auth
        if eff_token:
            normalized_headers["Authorization"] = f"Bearer {eff_token}"
        if api_key:
            normalized_headers["X-API-Key"] = api_key
        if headers:
            normalized_headers.update(headers)

        if normalized_headers:
            existing = dict(final_config.get("headers", {}) or {})
            existing.update(normalized_headers)
            final_config["headers"] = existing

        # 清理入口字段，避免持久化污染
        for k in ("token", "api_key", "auth"):
            if k in final_config:
                try:
                    del final_config[k]
                except Exception:
                    final_config.pop(k, None)

        return final_config


