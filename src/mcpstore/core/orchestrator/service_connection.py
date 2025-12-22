"""
MCPOrchestrator Service Connection Module

服务连接模块 - 通过事件驱动的标准流程处理服务连接

重要设计原则：
- 所有服务连接必须通过事件驱动的标准流程
- 不允许绕过 ServiceAddRequested -> ServiceCached -> ServiceConnected 流程
- 确保 service_metadata 在连接前被正确创建
"""

import logging
import json
from datetime import datetime
from typing import Dict, Any, Optional, Tuple

from fastmcp import Client

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class ServiceConnectionMixin:
    """
    服务连接混入类
    
    重要设计原则：
    - 所有服务连接必须通过事件驱动的标准流程
    - 不允许绕过 ServiceAddRequested -> ServiceCached -> ServiceConnected 流程
    - 确保 service_metadata 在连接前被正确创建
    """

    async def connect_service(self, name: str, service_config: Dict[str, Any] = None, url: str = None, agent_id: str = None) -> Tuple[bool, str]:
        """
        连接服务 - 通过事件驱动的标准流程
        
        重要：此方法不再直接连接服务，而是发布事件触发标准流程：
        - 如果服务不存在：发布 ServiceAddRequested 事件（触发完整流程）
        - 如果服务已存在：发布 ServiceConnectionRequested 事件（只触发连接）
        
        这确保了：
        1. service_metadata 在连接前被正确创建
        2. 所有缓存操作通过 CacheManager 统一处理
        3. 生命周期状态正确管理

        Args:
            name: 服务名称
            service_config: 服务配置（必须提供）
            url: 服务 URL（可选，会合并到 service_config）
            agent_id: Agent ID（可选，默认使用 global_agent_store_id）

        Returns:
            Tuple[bool, str]: (是否成功发布事件, 消息)
        """
        try:
            # 确定 Agent ID
            agent_key = agent_id or self.client_manager.global_agent_store_id
            
            # 获取服务配置
            if service_config is None:
                service_config = self.registry.get_service_config_from_cache(agent_key, name)
                if not service_config:
                    raise RuntimeError(
                        f"服务配置不存在: service_name={name}, agent_id={agent_key}. "
                        f"请先通过 add_service 添加服务配置。"
                    )
            
            # 合并 URL 参数
            if url:
                service_config = service_config.copy()
                service_config["url"] = url
            
            # 检查服务是否已存在（包括服务实体和元数据）
            # 必须同时检查服务实体和 service_metadata，确保数据一致性
            service_exists = await self.registry.has_service_async(agent_key, name)
            metadata_exists = False
            if service_exists:
                # 服务实体存在，检查 metadata 是否也存在
                metadata = await self.registry.get_service_metadata_async(agent_key, name)
                metadata_exists = metadata is not None
                if not metadata_exists:
                    logger.warning(
                        f"[CONNECT_SERVICE] 服务 {name} 实体存在但 metadata 不存在，"
                        f"数据不一致，将走完整的添加流程"
                    )

            # #region agent log
            try:
                log_payload = {
                    "sessionId": "debug-session",
                    "runId": "pre-fix",
                    "hypothesisId": "H2",
                    "location": "service_connection.py:connect_service",
                    "message": "connect_service state pre-branch",
                    "data": {
                        "agent_id": agent_key,
                        "service_name": name,
                        "service_exists": service_exists,
                        "metadata_exists": metadata_exists,
                        "config_keys": list(service_config.keys()) if service_config else [],
                    },
                    "timestamp": datetime.now().timestamp(),
                }
                with open("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log", "a") as f:
                    f.write(json.dumps(log_payload, ensure_ascii=False) + "\n")
            except Exception:
                pass
            # #endregion
            
            if not service_exists or not metadata_exists:
                # 服务不存在，走完整的添加流程
                logger.info(f"[CONNECT_SERVICE] 服务 {name} 不存在，发布 ServiceAddRequested 事件")
                
                from mcpstore.core.events.service_events import ServiceAddRequested
                from mcpstore.core.utils.id_generator import ClientIDGenerator
                
                client_id = ClientIDGenerator.generate_deterministic_id(
                    agent_id=agent_key,
                    service_name=name,
                    service_config=service_config,
                    global_agent_store_id=self.client_manager.global_agent_store_id
                )
                
                add_event = ServiceAddRequested(
                    agent_id=agent_key,
                    service_name=name,
                    service_config=service_config,
                    client_id=client_id,
                    source="connect_service"
                )
                
                # 同步等待事件处理完成
                await self.container._event_bus.publish(add_event, wait=True)
                logger.info(f"[CONNECT_SERVICE] ServiceAddRequested 事件已发布: {name}")
                return True, f"服务 {name} 添加请求已发布，正在处理中"
            else:
                # 服务已存在，只触发重新连接
                logger.info(f"[CONNECT_SERVICE] 服务 {name} 已存在，发布 ServiceConnectionRequested 事件")
                
                from mcpstore.core.events.service_events import ServiceConnectionRequested
                
                connection_event = ServiceConnectionRequested(
                    agent_id=agent_key,
                    service_name=name,
                    service_config=service_config,
                    timeout=3.0
                )
                
                # 同步等待事件处理完成
                await self.container._event_bus.publish(connection_event, wait=True)
                logger.info(f"[CONNECT_SERVICE] ServiceConnectionRequested 事件已发布: {name}")
                return True, f"服务 {name} 连接请求已发布，正在处理中"

        except Exception as e:
            logger.error(f"[CONNECT_SERVICE] 连接服务 {name} 失败: {e}")
            raise

    # ========================================
    # 以下方法已废弃，服务连接现在通过事件驱动流程处理
    # _connect_local_service, _connect_remote_service, _update_service_cache
    # 已删除，由 ConnectionManager 和 CacheManager 统一处理
    # ========================================

    async def disconnect_service(self, url_or_name: str) -> bool:
        """???????????global_agent_store"""
        logger.info(f"Removing service: {url_or_name}")

        # ?????????
        name_to_remove = None
        for name, server in self.global_agent_store_config.get("mcpServers", {}).items():
            if name == url_or_name or server.get("url") == url_or_name:
                name_to_remove = name
                break

        if name_to_remove:
            # ?global_agent_store_config???
            if name_to_remove in self.global_agent_store_config["mcpServers"]:
                del self.global_agent_store_config["mcpServers"][name_to_remove]

            # ????????
            ok = self.mcp_config.remove_service(name_to_remove)
            if not ok:
                logger.warning(f"Failed to remove service {name_to_remove} from configuration file")

            # ?registry???（使用异步版本）
            agent_id = self.client_manager.global_agent_store_id
            await self.registry.remove_service_async(agent_id, name_to_remove)

            # ????global_agent_store
            if self.global_agent_store_config.get("mcpServers"):
                self.global_agent_store = Client(self.global_agent_store_config)

                # ????agent_clients
                for agent_id in list(self.agent_clients.keys()):
                    self.agent_clients[agent_id] = Client(self.global_agent_store_config)
                    logger.info(f"Updated client for agent {agent_id} after removing service")

            else:
                # ??????????global_agent_store
                self.global_agent_store = None
                # ????agent_clients
                self.agent_clients.clear()

            return True
        else:
            logger.warning(f"Service {url_or_name} not found in configuration.")
            return False

    async def refresh_services(self):
        """???????????????mcp.json?"""
        #  ????????????????
        if hasattr(self, 'sync_manager') and self.sync_manager:
            await self.sync_manager.sync_global_agent_store_from_mcp_json()
        else:
            logger.warning("Sync manager not available, cannot refresh services")

    async def refresh_service_content(self, service_name: str, agent_id: str = None) -> bool:
        """刷新服务内容（工具、资源等）"""
        if self.content_manager is None:
            raise RuntimeError(
                f"content_manager 未初始化，无法刷新服务内容: service_name={service_name}"
            )
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return await self.content_manager.force_update_service_content(agent_key, service_name)

    async def is_service_healthy(self, name: str, client_id: Optional[str] = None) -> bool:
        """
        ??????????????????

        Args:
            name: ???
            client_id: ??????ID?????????

        Returns:
            bool: True ?????? HEALTHY/WARNING ??
        """
        agent_key = client_id or self.client_manager.global_agent_store_id
        state = self.registry._service_state_service.get_service_state(agent_key, name)
        return state in (ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING)

    def _normalize_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """?????????????????"""
        if not service_config:
            return service_config

        # ??????
        normalized = service_config.copy()

        # ????transport?????????
        if "url" in normalized and "transport" not in normalized:
            url = normalized["url"]
            if "/sse" in url.lower():
                normalized["transport"] = "sse"
            else:
                normalized["transport"] = "streamable-http"
            logger.debug(f"Auto-inferred transport type: {normalized['transport']} for URL: {url}")

        return normalized
