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
    """更新服务认证助手 - 明确的服务名，避免状态混乱
    
    Note: 这是一个内部助手类，为了符合 async-only 约束，
    所有方法都改为 async，外部调用者需要 await。
    """

    def __init__(self, context: 'MCPStoreContext', service_name: str, config: Dict[str, Any] = None):
        self._context = context
        self._service_name = service_name  # [CONFIG] Clear service name to avoid confusion
        self._config = config.copy() if config else {}

    async def bearer_auth(self, auth: str) -> 'MCPStoreContext':
        """Update Bearer Token authentication for specified service (backward compatible)"""
        # Standardize to Authorization header
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["Authorization"] = f"Bearer {auth}"
        return await self._execute_update()

    async def token(self, token: str) -> 'MCPStoreContext':
        """Recommended: Set Bearer Token (equivalent to bearer_auth)"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["Authorization"] = f"Bearer {token}"
        return await self._execute_update()

    async def api_key(self, api_key: str) -> 'MCPStoreContext':
        """Recommended: Set API Key (standardized to X-API-Key)"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"]["X-API-Key"] = api_key
        return await self._execute_update()

    async def custom_headers(self, headers: Dict[str, str]) -> 'MCPStoreContext':
        """Update custom headers for specified service (explicit override)"""
        if "headers" not in self._config:
            self._config["headers"] = {}
        self._config["headers"].update(headers)
        return await self._execute_update()

    async def _execute_update(self) -> 'MCPStoreContext':
        """执行更新服务（内部 async-only）"""
        await self._context.update_service_async(self._service_name, self._config)
        return self._context


class ServiceManagementMixin:
    """服务管理混入类"""

    # [已删除] check_services 同步方法
    # 根据 "pykv 唯一真相数据源" 原则，请使用 check_services_async 异步方法

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
            logger.error(f"[check_services] Unknown context type: {self._context_type}")
            return {}

    def get_service_info(self, name: str) -> Any:
        """
        获取服务详情（同步版本），支持 store/agent 上下文
        - store上下文：在 global_agent_store 下的所有 client 中查找服务
        - agent上下文：在指定 agent_id 下的所有 client 中查找服务

        [新架构] 避免_sync_helper.run_async，使用更安全的同步实现
        """
        try:
            if not name:
                return {}

            if self._context_type == ContextType.STORE:
                logger.debug(f"STORE mode - searching service in global_agent_store: {name}")
                agent_id = self._store.client_manager.global_agent_store_id
            else:
                logger.debug(f"AGENT mode - searching service in agent({self._agent_id}): {name}")
                agent_id = self._agent_id

            # 直接从缓存获取服务信息
            complete_info = self._store.registry.get_complete_service_info(agent_id, name)
            if not complete_info:
                logger.debug(f"Service {name} not found in agent {agent_id}")
                return {}

            # 构建返回信息
            return {
                "name": name,
                "client_id": complete_info.get("client_id"),
                "config": complete_info.get("config", {}),
                "state": complete_info.get("state", "disconnected"),
                "tool_count": complete_info.get("tool_count", 0),
                "agent_id": agent_id
            }

        except Exception as e:
            logger.error(f"[NEW_ARCH] get_service_info failed: {e}")
            return {
                "name": name,
                "error": str(e),
                "agent_id": getattr(self, '_agent_id', 'unknown')
            }

    # 别名：符合命名规范
    def service_info(self, name: str) -> Any:
        return self.get_service_info(name)

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
            logger.error(f"[get_service_info] Unknown context type: {self._context_type}")
            return {}

    async def service_info_async(self, name: str) -> Any:
        return await self.get_service_info_async(name)

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

            try:
                self._run_async_via_bridge(
                    self.update_service_async(name, final_config),
                    op_name="service_management.update_service"
                )
            except Exception as e:
                logger.error(f"[NEW_ARCH] update_service failed: {e}")
            return self
        else:
            # 没有配置参数：
            if any([auth, token, api_key, headers]):
                # 纯认证：立即执行（也走补丁合并语义）
                final_config = self._apply_auth_to_update_config({}, auth, token, api_key, headers)
                try:
                    self._run_async_via_bridge(
                        self.update_service_async(name, final_config),
                        op_name="service_management.update_service_auth"
                    )
                except Exception as e:
                    logger.error(f"[NEW_ARCH] update_service (auth) failed: {e}")
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
                    # 从 pykv 异步获取元数据
                    global_agent = self._store.client_manager.global_agent_store_id
                    metadata = await self._store.registry._service_state_service.get_service_metadata_async(global_agent, global_name)
                    if metadata:
                        # 将变更合并到缓存元数据中
                        metadata.service_config = _deep_merge(metadata.service_config or {}, config)
                        await self._store.registry.set_service_metadata_async(global_agent, global_name, metadata)
                except Exception as e:
                    logger.error(f"Failed to update service metadata: {e}")
                    raise

                return success
        except Exception as e:
            logger.error(f"Failed to update service {name}: {e}")
            raise

    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        """
        增量更新服务配置（同步版本）- 推荐使用

        Args:
            name: 服务名称
            updates: 要更新的配置项

        Returns:
            bool: 更新是否成功
        """
        try:
            return self._run_async_via_bridge(
                self.patch_service_async(name, updates),
                op_name="service_management.patch_service"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] patch_service failed: {e}")
            return False

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
                    # 从 pykv 异步获取元数据
                    global_agent = self._store.client_manager.global_agent_store_id
                    metadata = await self._store.registry._service_state_service.get_service_metadata_async(global_agent, global_name)
                    if metadata:
                        metadata.service_config.update(updates)
                        self._store.registry.set_service_metadata(global_agent, global_name, metadata)
                except Exception as e:
                    logger.error(f"Failed to update service metadata: {e}")
                    raise

                return success
        except Exception as e:
            logger.error(f"Failed to patch service {name}: {e}")
            raise

    def delete_service(self, name: str) -> bool:
        """
        删除服务（同步版本）

        Args:
            name: 服务名称

        Returns:
            bool: 删除是否成功
        """
        try:
            return self._run_async_via_bridge(
                self.delete_service_async(name),
                op_name="service_management.delete_service"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] delete_service failed: {e}")
            return False

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

    def _normalize_agent_local_name(self, input_name: str) -> str:
        """
        将输入的服务标识归一化为当前 Agent 的本地服务名。

        支持三种输入：
        1) 纯本地名（默认）
        2) 全局名格式：<local>_byagent_<agent_id>
        3) 冒号分隔：<agent_id>:<local>

        若提供的 agent_id 与当前上下文不一致则抛出异常，避免误删。
        """
        if not input_name:
            raise ValueError("service name is required")

        # 统一委托给 PerspectiveResolver，避免重复实现视角转换
        try:
            from mcpstore.utils.perspective_resolver import PerspectiveResolver

            resolver = PerspectiveResolver()
            res = resolver.normalize_service_name(
                self._agent_id,
                input_name,
                target="local",
                strict=True,
            )
            logger.debug(
                f"[PERSPECTIVE] normalize input='{input_name}' -> local='{res.local_name}' method='{res.resolution_method}'"
            )
            return res.local_name
        except Exception as e:
            logger.error(f"[PERSPECTIVE] normalize_agent_local_name failed: {e}")
            raise

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

    def reset_config(self) -> bool:
        """
        重置配置（同步版本）
        
        清空所有 pykv 缓存数据和 mcp.json 文件。
        相当于批量执行 delete_service 操作。
        """
        return self._run_async_via_bridge(
            self.reset_config_async(),
            op_name="service_management.reset_config"
        )

    def switch_cache(self, cache_config: Any) -> bool:
        """运行时切换缓存后端（同步版本）。

        仅支持 Store 上下文；Agent 上下文会抛出 ValueError。
        """
        try:
            return self._run_async_via_bridge(
                self.switch_cache_async(cache_config),
                op_name="service_management.switch_cache"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] switch_cache failed: {e}")
            return False

    async def switch_cache_async(self, cache_config: Any) -> bool:
        """运行时切换缓存后端（异步版本）。"""
        try:
            if self._context_type != ContextType.STORE:
                raise ValueError("Cache switching is only supported in STORE context")

            # 委托给 Store 层的封装方法，内部会进行配置解析和连接测试
            await self._store._switch_cache_backend(cache_config)
            return True
        except Exception as e:
            logger.error(f"Failed to switch cache backend: {e}")
            return False

    async def reset_config_async(self) -> bool:
        """
        重置配置（异步版本）
        
        清空所有 pykv 缓存数据和 mcp.json 文件。
        相当于批量执行 delete_service 操作。
        
        清理内容：
        - pykv 实体层：services, tools
        - pykv 关系层：agent_services, service_tools
        - pykv 状态层：service_status, service_metadata
        - mcp.json 文件
        - 健康检查任务（通过服务不存在检测自动停止）
        
        根据上下文类型执行不同的重置操作：
        - Store 上下文：清空所有 Agent 的配置
        - Agent 上下文：只清空该 Agent 的配置
        """
        if self._context_type == ContextType.STORE:
            return await self._reset_store_config()
        else:
            return await self._reset_agent_config()

    async def _reset_store_config(self) -> bool:
        """
        Store 级别重置配置
        
        清理流程：
        1. 获取所有 Agent ID
        2. 对每个 Agent 调用 registry.clear_async()
           - clear_async 内部调用 remove_service_async 逐个删除服务
           - remove_service_async 清理：实体层、关系层、状态层、工具实体
        3. 重置 mcp.json 为空配置
        """
        logger.info("[RESET_CONFIG] [STORE] Store level: starting to reset all configurations")
        
        # 1. 获取所有 Agent ID
        agent_ids = await self._store.registry.get_all_agent_ids_async()
        logger.debug(f"[RESET_CONFIG] [CLEAN] Found {len(agent_ids)} Agents need to be cleaned")
        
        # 2. 清空每个 Agent 的缓存数据
        for agent_id in agent_ids:
            logger.debug(f"[RESET_CONFIG] [CLEAN] Cleaning Agent: {agent_id}")
            await self._store.registry.clear_async(agent_id)
        
        # 3. 重置 mcp.json 文件
        default_config = {"mcpServers": {}}
        mcp_success = self._store._unified_config.update_mcp_config(default_config)
        
        logger.info("[RESET_CONFIG] [STORE] Store level: configuration reset completed")
        return mcp_success

    async def _reset_agent_config(self) -> bool:
        """Agent级别重置配置的内部实现"""
        try:
            logger.info(f"[RESET_CONFIG] [AGENT] Agent level: resetting all configurations for Agent {self._agent_id}")

            # 1. 清空Agent在缓存中的数据（使用异步版本）
            await self._store.registry.clear_async(self._agent_id)

            # 2. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f"[RESET_CONFIG] [AGENT] Agent level: Agent {self._agent_id} configuration reset completed")
            return True

        except Exception as e:
            logger.error(f"[RESET_CONFIG] [ERROR] Agent level configuration reset failed: {e}")
            return False

    def show_config(self) -> Dict[str, Any]:
        """
        显示配置信息（同步版本）

        - Store级别: 返回所有Agent的配置
        - Agent级别: 返回该Agent的配置

        Returns:
            Dict: 配置信息字典
        """
        try:
            return self._run_async_via_bridge(
                self.show_config_async(),
                op_name="service_management.show_config"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] show_config failed: {e}")
            return {}

    async def show_config_async(self) -> Dict[str, Any]:
        """
        显示配置信息（异步版本）- 遵循 Functional Core, Imperative Shell 架构

        架构说明：
        - 使用 ShowConfigAsyncShell 作为异步外壳，负责 pykv IO 操作
        - 使用 ShowConfigLogicCore 作为纯逻辑核心，负责数据组装
        - 严格遵循 pykv 唯一真相数据源原则

        根据上下文类型执行不同的显示操作：
        - Store上下文：显示所有Agent的配置
        - Agent上下文：显示该Agent的配置

        Returns:
            Dict: 配置信息字典
        """
        try:
            # 获取 CacheLayerManager 实例
            cache_layer = self._get_cache_layer_manager()
            
            # 创建异步外壳实例
            from mcpstore.core.architecture.show_config_shell import ShowConfigAsyncShell
            shell = ShowConfigAsyncShell(cache_layer)
            
            if self._context_type == ContextType.STORE:
                return await shell.show_store_config_async()
            else:
                return await shell.show_agent_config_async(self._agent_id)
                
        except Exception as e:
            logger.error(f"Failed to show config: {e}")
            # 使用纯逻辑核心构建错误响应
            from mcpstore.core.architecture.show_config_core import ShowConfigLogicCore
            logic_core = ShowConfigLogicCore()
            return logic_core.build_error_response(
                f"Failed to show config: {str(e)}",
                agent_id=self._agent_id if self._context_type != ContextType.STORE else None
            )

    def _get_cache_layer_manager(self):
        """
        获取 CacheLayerManager 实例
        
        遵循 pykv 唯一真相数据源原则，确保使用正确的缓存层管理器。
        
        Returns:
            CacheLayerManager 实例
            
        Raises:
            RuntimeError: 如果无法获取 CacheLayerManager
        """
        # 从 registry 获取 _cache_layer_manager
        # 注意：不再使用 _cache_layer，因为它在 Redis 模式下是 RedisStore，没有所需的方法
        cache_layer = getattr(self._store.registry, '_cache_layer_manager', None)
        if cache_layer is not None:
            return cache_layer
        
        # 尝试从 store 获取
        cache_layer = getattr(self._store, '_cache_layer_manager', None)
        if cache_layer is not None:
            return cache_layer
        
        raise RuntimeError(
            "无法获取 CacheLayerManager 实例。"
            "请确保 MCPStore 已正确初始化，且 registry._cache_layer_manager 已设置。"
        )

    def delete_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """
        删除服务配置（同步版本）

        Args:
            client_id_or_service_name: client_id或服务名

        Returns:
            Dict: 删除结果
        """
        try:
            return self._run_async_via_bridge(
                self.delete_config_async(client_id_or_service_name),
                op_name="service_management.delete_config"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] delete_config failed: {e}")
            return {
                "success": False,
                "error": str(e),
                "client_id": None,
                "service_name": None
            }

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
        try:
            return self._run_async_via_bridge(
                self.update_config_async(client_id_or_service_name, new_config),
                op_name="service_management.update_config"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] update_config failed: {e}")
            return {
                "success": False,
                "error": str(e),
                "client_id": None,
                "service_name": None,
                "old_config": None,
                "new_config": None
            }

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

    async def _validate_resolved_mapping_async(self, client_id: str, service_name: str, agent_id: str) -> bool:
        """
        验证解析后的client_id和service_name映射是否有效（异步版本）

        Args:
            client_id: 解析出的client_id
            service_name: 解析出的service_name
            agent_id: Agent ID

        Returns:
            bool: 映射是否有效
        """
        try:
            # 检查client_id是否存在于agent的映射中 - 从 pykv 获取
            agent_clients = await self._store.registry.get_agent_clients_async(agent_id)
            if client_id not in agent_clients:
                logger.debug(f" [VALIDATE_MAPPING] client_id '{client_id}' not found in agent '{agent_id}' clients")
                return False

            # 检查service_name是否存在于Registry中
            existing_client_id = await self._store.registry._agent_client_service.get_service_client_id_async(agent_id, service_name)
            if existing_client_id != client_id:
                logger.debug(f" [VALIDATE_MAPPING] service '{service_name}' maps to different client_id: expected={client_id}, actual={existing_client_id}")
                return False

            return True
        except Exception as e:
            logger.debug(f" [VALIDATE_MAPPING] Validation failed: {e}")
            return False

    def _validate_resolved_mapping(self, client_id: str, service_name: str, agent_id: str) -> bool:
        raise RuntimeError("[SERVICE_MANAGEMENT] Synchronous validate_mapping is disabled, please use _validate_resolved_mapping_async.")

    async def _resolve_client_id_async(self, client_id_or_service_name: str, agent_id: str) -> Tuple[str, str]:
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
                mapped = await self._store.registry.get_global_name_from_agent_service_async(agent_id, input_name)
                global_service_name = mapped or AgentServiceMapper(agent_id).to_global_name(input_name)

            # 2.2 优先在 Agent 命名空间解析 client_id，再回退到 Store 命名空间
            client_id = await self._store.registry._agent_client_service.get_service_client_id_async(agent_id, input_name)
            if not client_id:
                # 回退到 Store 命名空间
                client_id = await self._store.registry._agent_client_service.get_service_client_id_async(global_agent_id, global_service_name)

            if not client_id:
                available_agent = ', '.join(await self._store.registry.get_all_service_names_async(agent_id)) or 'None'
                available_global = ', '.join(await self._store.registry.get_all_service_names_async(global_agent_id)) or 'None'
                raise ValueError(
                    f"Service '{input_name}' (global '{global_service_name}') not found. "
                    f"Agent services: {available_agent}. Store services: {available_global}"
                )

            logger.debug(f"[RESOLVE_CLIENT_ID] agent_proxy_ok local_or_global='{input_name}' -> global='{global_service_name}' client_id={client_id}")
            return client_id, global_service_name

        # 3) Store 模式：直接在 Store 命名空间解析
        service_name = client_id_or_service_name
        service_names = await self._store.registry.get_all_service_names_async(agent_id)
        if service_name in service_names:
            client_id = await self._store.registry._agent_client_service.get_service_client_id_async(agent_id, service_name)
            if client_id:
                logger.debug(f"[RESOLVE_CLIENT_ID] store_lookup_ok service={service_name} client_id={client_id}")
                return client_id, service_name
            else:
                raise ValueError(f"Service '{service_name}' found but no client_id mapping")

        available_services = ', '.join(service_names) if service_names else 'None'
        raise ValueError(f"Service '{service_name}' not found in store. Available services: {available_services}")

    def _resolve_client_id(self, client_id_or_service_name: str, agent_id: str) -> Tuple[str, str]:
        """
        同步包装，保留给旧代码使用；内部通过 AOB 执行异步解析。
        """
        return self._run_async_via_bridge(
            self._resolve_client_id_async(client_id_or_service_name, agent_id),
            op_name="service_management.resolve_client_id"
        )

    async def _delete_store_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """Store级别删除配置的内部实现"""
        try:
            logger.info(f"[DELETE_CONFIG] [STORE] Store level: deleting configuration {client_id_or_service_name}")

            global_agent_store_id = self._store.client_manager.global_agent_store_id

            # 解析client_id和服务名
            client_id, service_name = await self._resolve_client_id_async(client_id_or_service_name, global_agent_store_id)

            logger.info(f"[DELETE_CONFIG] [RESOLVE] Resolution result: client_id={client_id}, service_name={service_name}")

            # 验证服务存在
            if not self._store.registry.get_session(global_agent_store_id, service_name):
                logger.warning(f"Service {service_name} not found in registry, but continuing with cleanup")

            # 事务性删除：先删除文件配置，再删除缓存
            # 1. 从mcp.json中删除服务配置（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.remove_service_config(service_name)
            if success:
                logger.info(f"[DELETE_CONFIG] [SUCCESS] Service removed from mcp.json: {service_name}, cache synchronized")

            # 2. 从缓存中删除服务（包括工具和会话）- 使用异步版本
            await self._store.registry.remove_service_async(global_agent_store_id, service_name)

            # 3. 删除Service-Client映射
            self._store.registry.remove_service_client_mapping(global_agent_store_id, service_name)

            # 4. 删除Client配置
            self._store.registry.remove_client_config(client_id)

            # 5. 删除Agent-Client映射
            self._store.registry.remove_agent_client_mapping(global_agent_store_id, client_id)

            # 6. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f"[DELETE_CONFIG] [STORE] Store level: configuration deletion completed {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' deleted successfully",
                "client_id": client_id,
                "service_name": service_name
            }

        except Exception as e:
            logger.error(f"[DELETE_CONFIG] [ERROR] Store level configuration deletion failed: {e}")
            return {
                "success": False,
                "error": f"Failed to delete store config: {str(e)}",
                "client_id": None,
                "service_name": None
            }

    async def _delete_agent_config(self, client_id_or_service_name: str) -> Dict[str, Any]:
        """Agent级别删除配置的内部实现"""
        try:
            logger.info(f"[DELETE_CONFIG] [AGENT] Agent level: deleting Agent {self._agent_id} configuration {client_id_or_service_name}")

            # 解析client_id和服务名
            client_id, service_name = await self._resolve_client_id_async(client_id_or_service_name, self._agent_id)

            logger.info(f"[DELETE_CONFIG] [RESOLVE] Resolution result: client_id={client_id}, service_name={service_name}")

            # 验证服务存在
            if not self._store.registry.get_session(self._agent_id, service_name):
                logger.warning(f"Service {service_name} not found in registry for agent {self._agent_id}, but continuing with cleanup")

            # Agent级别删除：只删除缓存，不修改mcp.json
            # 1. 从缓存中删除服务（包括工具和会话）- 使用异步版本
            await self._store.registry.remove_service_async(self._agent_id, service_name)

            # 2. 删除Service-Client映射
            self._store.registry.remove_service_client_mapping(self._agent_id, service_name)

            # 3. 删除Client配置
            self._store.registry.remove_client_config(client_id)

            # 4. 删除Agent-Client映射
            self._store.registry.remove_agent_client_mapping(self._agent_id, client_id)

            # 5. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")

            logger.info(f"[DELETE_CONFIG] [AGENT] Agent level: configuration deletion completed {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' deleted successfully from agent '{self._agent_id}'",
                "client_id": client_id,
                "service_name": service_name
            }

        except Exception as e:
            logger.error(f"[DELETE_CONFIG] [ERROR] Agent level configuration deletion failed: {e}")
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
            logger.info(f"[UPDATE_CONFIG] [STORE] Store level: updating configuration {client_id_or_service_name}")

            global_agent_store_id = self._store.client_manager.global_agent_store_id

            # 解析client_id和服务名
            client_id, service_name = await self._resolve_client_id_async(client_id_or_service_name, global_agent_store_id)

            logger.info(f"[UPDATE_CONFIG] [RESOLVE] Resolution result: client_id={client_id}, service_name={service_name}")

            # 获取当前配置
            old_complete_info = await self._store.registry.get_complete_service_info_async(global_agent_store_id, service_name)
            old_config = old_complete_info.get("config", {})

            if not old_config:
                raise ValueError(f"Service '{service_name}' configuration not found")

            # 验证和标准化新配置
            normalized_config = self._validate_and_normalize_config(new_config, service_name, old_config)

            logger.info(f"[UPDATE_CONFIG] [VALIDATE] Configuration validation passed, starting update: {service_name}")

            # 1. 清空服务的工具和会话数据
            self._store.registry.clear_service_tools_only(global_agent_store_id, service_name)

            # 2. 更新Client配置缓存
            self._store.registry.update_client_config(client_id, {
                "mcpServers": {service_name: normalized_config}
            })

            # 3. 设置服务状态为STARTUP并更新元数据
            from mcpstore.core.models.service import ServiceConnectionState
            await self._store.orchestrator.lifecycle_manager._transition_state(
                agent_id=global_agent_store_id,
                service_name=service_name,
                new_state=ServiceConnectionState.STARTUP,
                reason="config_updated",
                source="ServiceManagement",
            )

            # 从 pykv 异步获取并更新服务元数据中的配置
            metadata = await self._store.registry._service_state_service.get_service_metadata_async(global_agent_store_id, service_name)
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
            await self._store.orchestrator.lifecycle_manager.initialize_service(
                global_agent_store_id, service_name, normalized_config
            )

            logger.info(f"[UPDATE_CONFIG] [STORE] Store level: configuration update completed {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' configuration updated successfully",
                "client_id": client_id,
                "service_name": service_name,
                "old_config": old_config,
                "new_config": normalized_config
            }

        except Exception as e:
            logger.error(f"[UPDATE_CONFIG] [ERROR] Store level configuration update failed: {e}")
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
            logger.info(f"[UPDATE_CONFIG] [AGENT] Agent level: updating Agent {self._agent_id} configuration {client_id_or_service_name}")

            # 解析client_id和服务名
            client_id, service_name = await self._resolve_client_id_async(client_id_or_service_name, self._agent_id)

            logger.info(f"[UPDATE_CONFIG] [RESOLVE] Resolution result: client_id={client_id}, service_name={service_name}")

            # 获取当前配置
            old_complete_info = await self._store.registry.get_complete_service_info_async(self._agent_id, service_name)
            old_config = old_complete_info.get("config", {})

            if not old_config:
                raise ValueError(f"Service '{service_name}' configuration not found")

            # 验证和标准化新配置
            normalized_config = self._validate_and_normalize_config(new_config, service_name, old_config)

            logger.info(f"[UPDATE_CONFIG] [VALIDATE] Configuration validation passed, starting update: {service_name}")

            # 1. 清空服务的工具和会话数据
            self._store.registry.clear_service_tools_only(self._agent_id, service_name)

            # 2. 更新Client配置缓存
            self._store.registry.update_client_config(client_id, {
                "mcpServers": {service_name: normalized_config}
            })

            # 3. 设置服务状态为STARTUP并更新元数据
            from mcpstore.core.models.service import ServiceConnectionState
            await self._store.orchestrator.lifecycle_manager._transition_state(
                agent_id=self._agent_id,
                service_name=service_name,
                new_state=ServiceConnectionState.STARTUP,
                reason="agent_config_updated",
                source="ServiceManagement",
            )

            # 从 pykv 异步获取并更新服务元数据中的配置
            metadata = await self._store.registry._service_state_service.get_service_metadata_async(self._agent_id, service_name)
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
            await self._store.orchestrator.lifecycle_manager.initialize_service(
                self._agent_id, service_name, normalized_config
            )

            logger.info(f"[UPDATE_CONFIG] [AGENT] Agent level: configuration update completed {service_name}")

            return {
                "success": True,
                "message": f"Service '{service_name}' configuration updated successfully for agent '{self._agent_id}'",
                "client_id": client_id,
                "service_name": service_name,
                "old_config": old_config,
                "new_config": normalized_config
            }

        except Exception as e:
            logger.error(f"[UPDATE_CONFIG] [ERROR] Agent level configuration update failed: {e}")
            return {
                "success": False,
                "error": f"Failed to update agent config: {str(e)}",
                "client_id": None,
                "service_name": None,
                "old_config": None,
                "new_config": None
            }

    def get_service_status(self, name: str) -> dict:
        """获取单个服务的状态信息（同步版本，内部桥接异步）。"""
        try:
            return self._run_async_via_bridge(
                self.get_service_status_async(name),
                op_name="service_management.get_service_status"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] get_service_status failed: {e}")
            return {"status": "error", "error": str(e)}

    async def get_service_status_async(self, name: str) -> dict:
        """获取单个服务的状态信息"""
        try:
            if self._context_type == ContextType.STORE:
                return await self._store.orchestrator.get_service_status_async(name)
            else:
                # Agent模式：转换服务名称
                global_name = name
                if self._service_mapper:
                    global_name = self._service_mapper.to_global_name(name)
                # 透明代理：在全局命名空间查询状态
                return await self._store.orchestrator.get_service_status_async(global_name)
        except Exception as e:
            logger.error(f"Failed to get service status for {name}: {e}")
            return {"status": "error", "error": str(e)}

    # 别名：符合命名规范
    def service_status(self, name: str) -> dict:
        return self.get_service_status(name)

    async def service_status_async(self, name: str) -> dict:
        return await self.get_service_status_async(name)

    def restart_service(self, name: str) -> bool:
        raise RuntimeError("[SERVICE_MANAGEMENT] Synchronous restart_service is disabled, please use restart_service_async.")

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
        raise RuntimeError("[SERVICE_MANAGEMENT] Synchronous disconnect_service is disabled, please use disconnect_service_async.")

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
                # 尝试从映射关系中获取全局名称（使用异步版本，避免 AOB 事件循环冲突）
                global_name = await self._store.registry.get_global_name_from_agent_service_async(self._agent_id, local_name)
                if global_name:
                    logger.debug(f" [SERVICE_PROXY] Service name mapping: {local_name} -> {global_name}")
                    return global_name

            # 如果映射失败，可能是 Store 原生服务，直接返回
            logger.debug(f" [SERVICE_PROXY] No mapping, using original name: {local_name}")
            return local_name

        except Exception as e:
            logger.error(f" [SERVICE_PROXY] Service name mapping failed: {e}")
            return local_name

    async def _delete_store_service_with_sync(self, service_name: str):
        """Store 服务删除（带双向同步）"""
        try:
            # 1. 从 Registry 中删除（使用异步版本）
            await self._store.registry.remove_service_async(
                self._store.client_manager.global_agent_store_id,
                service_name
            )

            # 2. 从 mcp.json 中删除（使用 UnifiedConfigManager 自动刷新缓存）
            success = self._store._unified_config.remove_service_config(service_name)
            
            if success:
                    logger.info(f"[SERVICE_DELETE] [STORE] Store service deletion successful: {service_name}, cache synchronized")
            else:
                logger.error(f" [SERVICE_DELETE] Store service deletion failed: {service_name}")

            # 3. 触发双向同步（如果是 Agent 服务）
            if hasattr(self._store, 'bidirectional_sync_manager'):
                await self._store.bidirectional_sync_manager.handle_service_deletion_with_sync(
                    self._store.client_manager.global_agent_store_id,
                    service_name
                )

        except Exception as e:
            logger.error(f" [SERVICE_DELETE] Store service deletion failed {service_name}: {e}")
            raise

    async def _delete_agent_service_with_sync(self, local_name: str):
        """Agent 服务删除（带双向同步），返回是否成功"""
        try:
            # 宽容输入：支持本地名、全局名或 "agent:service" 格式
            local_name = self._normalize_agent_local_name(local_name)

            success = True
            # 1. 获取全局名称（使用异步版本，避免 AOB 事件循环冲突）
            global_name = await self._store.registry.get_global_name_from_agent_service_async(self._agent_id, local_name)
            if not global_name:
                logger.warning(f" [SERVICE_DELETE] Mapping not found: {self._agent_id}:{local_name}")
                return False

            # 2. 从 Agent 缓存中删除（使用异步版本）
            await self._store.registry.remove_service_async(self._agent_id, local_name)

            # 3. 从 Store 缓存中删除（使用异步版本）
            await self._store.registry.remove_service_async(
                self._store.client_manager.global_agent_store_id,
                global_name
            )

            # 4. 移除映射关系（仅映射表，不触发关系/状态删除）
            await self._store.registry.remove_agent_service_mapping_async(self._agent_id, local_name)

            # 5. 从 mcp.json 中删除（使用 UnifiedConfigManager 自动刷新缓存）
            success = success and self._store._unified_config.remove_service_config(global_name)
            
            if success:
                logger.info(f"[SERVICE_DELETE] [AGENT] Agent service deletion successful: {local_name} -> {global_name}, cache synchronized")
            else:
                logger.error(f" [SERVICE_DELETE] Agent service deletion failed: {local_name} -> {global_name}")

            # 6. 清理服务状态数据
            try:
                state_manager = self._store.registry._cache_state_manager
                await state_manager.delete_service_status(global_name)
                await state_manager.delete_service_metadata(global_name)
                logger.info(
                    f"[SERVICE_DELETE] Service status cleanup successful: "
                    f"agent_id={self._agent_id}, service={local_name}, global_name={global_name}"
                )
            except Exception as cleanup_error:
                logger.error(
                    f"[SERVICE_DELETE] Service status cleanup failed: "
                    f"agent_id={self._agent_id}, service={local_name}, error={cleanup_error}"
                )
                raise

            # 7. 单源模式：不再同步到分片文件
            logger.info("Single-source mode: skip shard mapping files sync")
            return success

        except Exception as e:
            logger.error(f" [SERVICE_DELETE] Agent service deletion failed {self._agent_id}:{local_name}: {e}")
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
            return self._run_async_via_bridge(
                self._show_config_agent_async(),
                op_name="service_management.show_config_agent"
            )

    async def _show_config_agent_async(self) -> Dict[str, Any]:
        """Agent上下文的 show_config 异步实现"""
        agent_id = self._agent_id
        # 从 pykv 获取 client_ids
        client_ids = await self._store.registry.get_agent_clients_async(agent_id)

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
                    raise_on_timeout: bool = False) -> Dict[str, Any]:
        """
        等待服务达到指定状态（同步版本，使用新架构避免死锁）。

        Args:
            client_id_or_service_name: client_id或服务名（智能识别）
            status: 目标状态，可以是单个状态字符串或状态列表
            timeout: 超时时间（秒），默认10秒
            raise_on_timeout: 超时时是否抛出异常，默认False

        Returns:
            dict: {success, status, retries_remaining, hard_timeout_remaining, last_error, window_metrics}

        Raises:
            TimeoutError: 当raise_on_timeout=True且超时时抛出
            ValueError: 当参数无法解析时抛出
        """
        try:
            # 解析服务名称（简化版，实际可能需要更复杂的解析逻辑）
            service_name = self._extract_service_name_from_identifier(client_id_or_service_name)

            # 使用新架构：同步外壳
            if not hasattr(self, '_service_management_sync_shell'):
                from ..architecture import ServiceManagementFactory
                self._service_management_sync_shell, _, _ = ServiceManagementFactory.create_service_management(
                    self._store.registry,
                    self._store.orchestrator,
                    agent_id=self._agent_id or self._store.client_manager.global_agent_store_id
                )

            # 直接调用同步外壳，避免_sync_helper.run_async的复杂性
            result = self._service_management_sync_shell.wait_service(service_name, timeout)
            if isinstance(result, bool):
                result = {"success": bool(result)}
            if not result.get("success") and raise_on_timeout:
                raise TimeoutError(f"Service {service_name} did not reach status {status} within {timeout} seconds")
            return result

        except Exception as e:
            logger.error(f"[NEW_ARCH] wait_service failed: {e}")
            if raise_on_timeout:
                raise
            return {"success": False, "error": str(e)}

    async def wait_service_async(self, client_id_or_service_name: str,
                               status: Union[str, List[str]] = 'healthy',
                               timeout: float = 10.0,
                               raise_on_timeout: bool = False) -> Dict[str, Any]:
        """
        等待服务达到指定状态（异步版本）

        Args:
            client_id_or_service_name: client_id或服务名（智能识别）
            status: 目标状态，可以是单个状态字符串或状态列表
            timeout: 超时时间（秒），默认10秒
            raise_on_timeout: 超时时是否抛出异常，默认False

        Returns:
            dict: {success, status, retries_remaining, hard_timeout_remaining, last_error, window_metrics}

        Raises:
            TimeoutError: 当raise_on_timeout=True且超时时抛出
            ValueError: 当参数无法解析时抛出
        """
        try:
            # 解析参数
            agent_scope = self._agent_id if self._context_type == ContextType.AGENT else self._store.client_manager.global_agent_store_id
            client_id, service_name = await self._resolve_client_id_async(client_id_or_service_name, agent_scope)

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
                    initial_status = (await self._store.orchestrator.get_service_status_async(service_name, status_agent_key) or {}).get("status", "unknown")
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
            last_meta: Dict[str, Any] = {}

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
                    return {
                        "success": False,
                        "status": prev_status or "unknown",
                        "hard_timeout_remaining": max(timeout - elapsed, 0.0),
                        "last_error": msg,
                    }

                # 获取当前状态（先读一次缓存，随后在必要时读一次新缓存以防止竞态）
                try:

                    status_dict = await self._store.orchestrator.get_service_status_async(service_name, status_agent_key) or {}
                    current_status = status_dict.get("status", "unknown")
                    try:
                        meta = await self._store.registry._service_state_service.get_service_metadata_async(status_agent_key, service_name)
                        if meta:
                            last_meta = {
                                "window_error_rate": getattr(meta, "window_error_rate", None),
                                "latency_p95": getattr(meta, "latency_p95", None),
                                "latency_p99": getattr(meta, "latency_p99", None),
                                "sample_size": getattr(meta, "sample_size", None),
                                "next_retry_time": getattr(meta, "next_retry_time", None),
                                "hard_deadline": getattr(meta, "hard_deadline", None),
                                "last_error": getattr(meta, "error_message", None),
                            }
                    except Exception as meta_err:
                        logger.debug(f"[WAIT_SERVICE] metadata_fetch_error service='{service_name}' error={meta_err}")

                    # 仅在状态变化或每2秒节流一次打印
                    now = time.time()
                    if current_status != prev_status or (now - last_log) > 2.0:
                        logger.debug(f"[WAIT_SERVICE] status service='{service_name}' value='{current_status}'")
                        # 对比 orchestrator 与 registry 的状态及最近健康检查（节流打印）
                        try:
                            reg_state = await self._store.registry.get_service_state_async(status_agent_key, service_name)
                            meta = await self._store.registry._service_state_service.get_service_metadata_async(status_agent_key, service_name)
                            last_check_ts = meta.last_health_check.isoformat() if getattr(meta, 'last_health_check', None) else None
                            logger.debug(f"[WAIT_SERVICE] compare orchestrator='{current_status}' registry='{getattr(reg_state,'value',reg_state)}' last_check={last_check_ts}")
                        except Exception as e:
                            logger.debug(f"[WAIT_SERVICE] Failed to get metadata: {e}")

                        prev_status, last_log = current_status, now

                    if change_mode:
                        if current_status != initial_status:
                            logger.info(f"[WAIT_SERVICE] done mode=change service='{service_name}' from='{initial_status}' to='{current_status}' elapsed={elapsed:.2f}s")
                            return {
                                "success": True,
                                "status": current_status,
                                "window_metrics": {
                                    "error_rate": last_meta.get("window_error_rate") if last_meta else None,
                                    "latency_p95": last_meta.get("latency_p95") if last_meta else None,
                                    "latency_p99": last_meta.get("latency_p99") if last_meta else None,
                                    "sample_size": last_meta.get("sample_size") if last_meta else None,
                                } if last_meta else None,
                                "retry_in": self._remaining_seconds(last_meta.get("next_retry_time")) if last_meta else None,
                                "hard_timeout_in": self._remaining_seconds(last_meta.get("hard_deadline")) if last_meta else None,
                                "last_error": last_meta.get("last_error") if last_meta else None,
                            }
                    else:
                        # 检查是否达到目标状态
                        if current_status in target_statuses:
                            logger.info(f"[WAIT_SERVICE] done mode=target service='{service_name}' reached='{current_status}' elapsed={elapsed:.2f}s")
                            return {
                                "success": True,
                                "status": current_status,
                                "window_metrics": {
                                    "error_rate": last_meta.get("window_error_rate") if last_meta else None,
                                    "latency_p95": last_meta.get("latency_p95") if last_meta else None,
                                    "latency_p99": last_meta.get("latency_p99") if last_meta else None,
                                    "sample_size": last_meta.get("sample_size") if last_meta else None,
                                } if last_meta else None,
                                "retry_in": self._remaining_seconds(last_meta.get("next_retry_time")) if last_meta else None,
                                "hard_timeout_in": self._remaining_seconds(last_meta.get("hard_deadline")) if last_meta else None,
                                "last_error": last_meta.get("last_error") if last_meta else None,
                            }
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
            return {"success": False, "error": str(e)}

    @staticmethod
    def _remaining_seconds(dt: Any) -> Optional[float]:
        """计算距离未来时间的剩余秒数"""
        try:
            if dt is None:
                return None
            import datetime
            if isinstance(dt, (int, float)):
                return max(dt - time.time(), 0.0)
            if isinstance(dt, datetime.datetime):
                return max((dt - datetime.datetime.now()).total_seconds(), 0.0)
            return None
        except Exception:
            return None

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

    def _extract_service_name_from_identifier(self, client_id_or_service_name: str) -> str:
        """
        从标识符中提取服务名称（新架构辅助方法）

        Args:
            client_id_or_service_name: client_id或服务名

        Returns:
            str: 服务名称
        """
        if not isinstance(client_id_or_service_name, str):
            raise ValueError(f"Identifier must be a string, actual type: {type(client_id_or_service_name)}")

        # 如果包含client_id格式，提取服务名称
        if "::" in client_id_or_service_name:
            # global_agent_store::service_name 格式
            parts = client_id_or_service_name.split("::", 1)
            if len(parts) == 2:
                return parts[1]

        # 如果是client_id格式，提取服务名称
        if client_id_or_service_name.startswith("client_"):
            # client_global_agent_store_service_name 格式
            parts = client_id_or_service_name.split("_", 3)
            if len(parts) >= 4:
                return parts[3]

        # 直接返回作为服务名称
        return client_id_or_service_name
