"""
Mapping Manager - 映射管理模块

负责处理各种映射关系，包括：
1. 服务客户端映射
2. Agent服务映射
3. 客户端配置管理
4. 映射关系的创建、查询和删除
"""

import logging
from typing import Dict, Any, Optional, List, Tuple

logger = logging.getLogger(__name__)


class MappingManager:
    """
    映射管理器实现

    职责：
    - 管理服务与客户端的映射关系
    - 处理Agent与服务的映射
    - 管理客户端配置信息
    - 提供映射关系的查询功能
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        self._cache_layer = cache_layer
        self._naming = naming_service
        self._namespace = namespace

        # 映射缓存
        self._service_client_mapping = {}  # agent_id:service_name -> client_id
        self._agent_service_mapping = {}  # agent_id:local_name -> global_name
        self._client_config = {}  # client_id -> config

        # 全局名称反向映射
        self._global_name_mapping = {}  # global_name -> (agent_id, local_name)

        self._logger = logging.getLogger(self.__class__.__name__)
        self._logger.info(f"[MAPPING_MANAGER] [INIT] Initializing MappingManager, namespace: {namespace}")

    def initialize(self) -> None:
        """初始化映射管理器"""
        self._logger.info("[MAPPING_MANAGER] [INIT] MappingManager initialization completed")

    def cleanup(self) -> None:
        """清理映射管理器资源"""
        try:
            # 清理所有缓存
            self._service_client_mapping.clear()
            self._agent_service_mapping.clear()
            self._client_config.clear()
            self._global_name_mapping.clear()

            self._logger.info("[MAPPING_MANAGER] [CLEAN] MappingManager cleanup completed")
        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] MappingManager cleanup error: {e}")
            raise

    async def get_service_client_id_async(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        异步获取服务客户端ID

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            客户端ID或None
        """
        # 异步方法，内部调用同步实现
        return self.get_service_client_id(agent_id, service_name)

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务客户端ID

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            客户端ID或None
        """
        try:
            # 从缓存获取
            cache_key = f"{agent_id}:{service_name}"
            return self._service_client_mapping.get(cache_key)

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to get service client ID {agent_id}:{service_name}: {e}")
            return None

    async def get_agent_clients_async(self, agent_id: str) -> List[str]:
        """
        从 pykv 关系层获取 Agent 的所有客户端
        
        [pykv 唯一真相源] 所有数据必须从 pykv 读取，不允许绕过。

        Args:
            agent_id: Agent ID

        Returns:
            客户端ID列表
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        # 从 pykv 关系层获取 Agent 的服务列表
        relation_data = await self._cache_layer.get_relation(
            "agent_services",
            agent_id
        )
        
        if relation_data is None:
            self._logger.debug(f"[MAPPING] [INFO] No Agent relationship in pykv: agent_id={agent_id}")
            return []
        
        # 提取 client_ids
        services = relation_data.get("services", [])
        clients = []
        for svc in services:
            client_id = svc.get("client_id")
            if client_id:
                clients.append(client_id)
        
        # 去重
        unique_clients = list(set(clients))
        
        self._logger.debug(
            f"[MAPPING] Getting Agent clients from pykv: agent_id={agent_id}, "
            f"count={len(unique_clients)}"
        )
        
        return unique_clients

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """
        从缓存获取客户端配置

        Args:
            client_id: 客户端ID

        Returns:
            客户端配置或None
        """
        try:
            return self._client_config.get(client_id)

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to get client configuration {client_id}: {e}")
            return None

    def add_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        """
        添加客户端配置

        Args:
            client_id: 客户端ID
            config: 客户端配置
        """
        try:
            self._client_config[client_id] = config
            self._logger.debug(f"[MAPPING_MANAGER] [ADD] Added client configuration: {client_id}")

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to add client configuration {client_id}: {e}")

    def set_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        """
        设置服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client_id: 客户端ID
        """
        try:
            cache_key = f"{agent_id}:{service_name}"
            self._service_client_mapping[cache_key] = client_id
            self._logger.debug(f"[MAPPING_MANAGER] [SET] Set service client mapping: {cache_key} -> {client_id}")

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to set service client mapping {agent_id}:{service_name}: {e}")

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        """
        移除服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        try:
            cache_key = f"{agent_id}:{service_name}"
            if cache_key in self._service_client_mapping:
                del self._service_client_mapping[cache_key]
                self._logger.debug(f"[MAPPING_MANAGER] [REMOVE] Removed service client mapping: {cache_key}")

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to remove service client mapping {agent_id}:{service_name}: {e}")

    def set_service_client_mapping_async(self, agent_id: str, service_name: str, client_id: str) -> None:
        """
        异步设置服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client_id: 客户端ID
        """
        # 简化实现：同步调用
        self.set_service_client_mapping(agent_id, service_name, client_id)

    def delete_service_client_mapping_async(self, agent_id: str, service_name: str) -> None:
        """
        异步删除服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        # 简化实现：同步调用
        self.remove_service_client_mapping(agent_id, service_name)

    def add_agent_service_mapping(self, agent_id: str, local_name: str, global_name: str) -> None:
        """
        添加Agent服务映射

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            global_name: 全局服务名称
        """
        try:
            cache_key = f"{agent_id}:{local_name}"
            self._agent_service_mapping[cache_key] = global_name
            self._global_name_mapping[global_name] = (agent_id, local_name)
            self._logger.debug(f"[MAPPING_MANAGER] [ADD] Added Agent service mapping: {cache_key} -> {global_name}")

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to add Agent service mapping {agent_id}:{local_name}: {e}")

    def get_global_name_from_agent_service(self, agent_id: str, local_name: str) -> Optional[str]:
        """
        从Agent服务获取全局名称

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称

        Returns:
            全局名称或None
        """
        try:
            cache_key = f"{agent_id}:{local_name}"
            return self._agent_service_mapping.get(cache_key)

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to get global name {agent_id}:{local_name}: {e}")
            return None

    def get_global_name_from_agent_service_async(self, agent_id: str, local_name: str) -> Optional[str]:
        """
        异步从Agent服务获取全局名称

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称

        Returns:
            全局名称或None
        """
        # 简化实现：同步调用
        return self.get_global_name_from_agent_service(agent_id, local_name)

    def get_agent_service_from_global_name(self, global_name: str) -> Optional[Tuple[str, str]]:
        """
        从全局名称获取Agent服务

        Args:
            global_name: 全局名称

        Returns:
            (agent_id, local_name) 元组或None
        """
        try:
            return self._global_name_mapping.get(global_name)

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to get Agent service {global_name}: {e}")
            return None

    def get_agent_services(self, agent_id: str) -> List[str]:
        """
        获取Agent的所有服务

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        try:
            services = []
            prefix = f"{agent_id}:"

            for cache_key, global_name in self._agent_service_mapping.items():
                if cache_key.startswith(prefix):
                    local_name = cache_key.split(":", 1)[1]
                    services.append(local_name)

            return services

        except Exception as e:
            self._logger.error(f"Failed to get Agent service {agent_id}: {e}")
            return []

    def is_agent_service(self, global_name: str) -> bool:
        """
        检查是否为Agent服务

        Args:
            global_name: 全局名称

        Returns:
            是否为Agent服务
        """
        try:
            return global_name in self._global_name_mapping

        except Exception as e:
            self._logger.error(f"[MAPPING_MANAGER] [ERROR] Failed to check Agent service {global_name}: {e}")
            return False

    def remove_agent_service_mapping(self, agent_id: str, local_name: str):
        """
        移除Agent服务映射

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
        """
        try:
            cache_key = f"{agent_id}:{local_name}"

            # 获取全局名称
            global_name = self._agent_service_mapping.get(cache_key)

            # 移除映射
            if cache_key in self._agent_service_mapping:
                del self._agent_service_mapping[cache_key]

            # 移除反向映射
            if global_name and global_name in self._global_name_mapping:
                del self._global_name_mapping[global_name]

            self._logger.debug(f"Removing Agent service mapping: {cache_key}")

        except Exception as e:
            self._logger.error(f"Failed to remove Agent service mapping {agent_id}:{local_name}: {e}")

    def clear_agent_mappings(self, agent_id: str):
        """
        清除Agent的所有映射

        Args:
            agent_id: Agent ID
        """
        try:
            # 移除服务客户端映射
            prefix = f"{agent_id}:"
            keys_to_remove = []

            for cache_key in self._service_client_mapping:
                if cache_key.startswith(prefix):
                    keys_to_remove.append(cache_key)

            for key in keys_to_remove:
                del self._service_client_mapping[key]

            # 移除Agent服务映射
            keys_to_remove = []
            for cache_key in self._agent_service_mapping:
                if cache_key.startswith(prefix):
                    keys_to_remove.append(cache_key)

            for key in keys_to_remove:
                global_name = self._agent_service_mapping[key]
                del self._agent_service_mapping[key]

                # 移除反向映射
                if global_name in self._global_name_mapping:
                    del self._global_name_mapping[global_name]

            self._logger.info(f"Cleared all Agent mappings: {agent_id}")

        except Exception as e:
            self._logger.error(f"Failed to clear Agent mappings {agent_id}: {e}")

    def clear_all_mappings(self):
        """
        清除所有映射
        """
        try:
            self._service_client_mapping.clear()
            self._agent_service_mapping.clear()
            self._client_config.clear()
            self._global_name_mapping.clear()

            self._logger.info("Cleared all mappings")

        except Exception as e:
            self._logger.error(f"Failed to clear all mappings: {e}")

    def get_mapping_stats(self) -> Dict[str, Any]:
        """
        获取映射统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "service_client_mappings": len(self._service_client_mapping),
            "agent_service_mappings": len(self._agent_service_mapping),
            "clients": len(self._client_config),
            "global_name_mappings": len(self._global_name_mapping)
        }
