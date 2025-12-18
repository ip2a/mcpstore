"""
ShowConfigAsyncShell - show_config 的异步外壳

遵循 "Functional Core, Imperative Shell" 架构原则：
- 负责所有 IO 操作（pykv 读取）
- 只使用 await，不使用 asyncio.run()
- 在现有事件循环中执行
- 调用纯逻辑核心进行数据处理

返回格式说明：
show_config 返回类似 mcp.json 的格式：
{
    "mcpServers": {
        "context7": {"url": "https://mcp.context7.com/mcp"},
        "grep": {"url": "https://mcp.grep.app"}
    }
}
"""

import logging
from typing import Dict, Any, TYPE_CHECKING

from .show_config_core import ShowConfigLogicCore

if TYPE_CHECKING:
    from mcpstore.core.cache.cache_layer_manager import CacheLayerManager

logger = logging.getLogger(__name__)


class ShowConfigAsyncShell:
    """
    show_config 的异步外壳
    
    职责：
    - 从 pykv 读取所有需要的数据
    - 调用纯逻辑核心处理数据
    - 返回类似 mcp.json 格式的配置
    
    严格约束：
    - 只使用 await，不使用 asyncio.run()
    - 所有 pykv 操作在此层完成
    - 不包含业务逻辑计算
    """
    
    def __init__(self, cache_layer: 'CacheLayerManager', namespace: str = "default"):
        """
        初始化异步外壳
        
        Args:
            cache_layer: CacheLayerManager 实例
            namespace: 命名空间
        """
        self._cache_layer = cache_layer
        self._namespace = namespace
        self._logic_core = ShowConfigLogicCore()
    
    async def show_store_config_async(self) -> Dict[str, Any]:
        """
        异步获取 Store 级别配置（类似 mcp.json 格式）
        
        执行流程：
        1. 从 pykv 异步读取所有服务实体
        2. 提取服务配置
        3. 调用纯逻辑核心组装 mcpServers 格式
        
        Returns:
            类似 mcp.json 的配置:
            {
                "mcpServers": {
                    "context7": {"url": "https://mcp.context7.com/mcp"},
                    "grep": {"url": "https://mcp.grep.app"}
                }
            }
        """
        try:
            logger.info("[SHOW_CONFIG_SHELL] Store 级别：开始获取配置")
            
            # Step 1: 从 pykv 读取所有服务实体
            services_data = await self._read_all_services_data_async()
            
            # Step 2: 调用纯逻辑核心组装配置
            result = self._logic_core.build_store_config(services_data)
            
            logger.info(
                f"[SHOW_CONFIG_SHELL] Store 级别配置获取完成: "
                f"services={len(result.get('mcpServers', {}))}"
            )
            
            return result
            
        except Exception as e:
            logger.error(f"[SHOW_CONFIG_SHELL] Store 级别获取配置失败: {e}")
            return self._logic_core.build_error_response(
                f"Failed to show store config: {str(e)}"
            )
    
    async def show_agent_config_async(self, agent_id: str) -> Dict[str, Any]:
        """
        异步获取 Agent 级别配置（类似 mcp.json 格式）
        
        执行流程：
        1. 从 pykv 异步检查 Agent 是否存在
        2. 从 pykv 异步读取该 Agent 的服务数据
        3. 调用纯逻辑核心组装 mcpServers 格式
        
        Args:
            agent_id: Agent ID
        
        Returns:
            类似 mcp.json 的配置（带 agent_id）:
            {
                "agent_id": "...",
                "mcpServers": {
                    "context7": {"url": "https://mcp.context7.com/mcp"}
                }
            }
        """
        try:
            logger.info(f"[SHOW_CONFIG_SHELL] Agent 级别：开始获取 Agent {agent_id} 的配置")
            
            # Step 1: 从 pykv 检查 Agent 是否存在
            agent_exists = await self._check_agent_exists_async(agent_id)
            if not agent_exists:
                logger.warning(f"[SHOW_CONFIG_SHELL] Agent {agent_id} 不存在")
                return self._logic_core.build_error_response(
                    f"Agent '{agent_id}' not found",
                    agent_id=agent_id
                )
            
            # Step 2: 从 pykv 读取该 Agent 的服务数据
            services_data = await self._read_agent_services_data_async(agent_id)
            
            # Step 3: 调用纯逻辑核心组装配置
            result = self._logic_core.build_agent_config(agent_id, services_data)
            
            logger.info(
                f"[SHOW_CONFIG_SHELL] Agent {agent_id} 配置获取完成: "
                f"services={len(result.get('mcpServers', {}))}"
            )
            
            return result
            
        except Exception as e:
            logger.error(f"[SHOW_CONFIG_SHELL] Agent {agent_id} 获取配置失败: {e}")
            return self._logic_core.build_error_response(
                f"Failed to show agent config: {str(e)}",
                agent_id=agent_id
            )
    
    async def _read_all_services_data_async(self) -> Dict[str, Dict[str, Any]]:
        """
        从 pykv 异步读取所有服务数据
        
        遵循 pykv 唯一真相数据源原则，直接从 pykv 实体层读取。
        
        Returns:
            所有服务的配置数据
            格式: {service_original_name: {"config": {...}, "source_agent": "..."}}
        """
        services_data = {}
        
        try:
            # 从 pykv 实体层读取所有服务实体
            all_services = await self._cache_layer.get_all_entities_async("services")
            
            logger.debug(f"[SHOW_CONFIG_SHELL] 从 pykv 读取到 {len(all_services)} 个服务实体")
            
            # 提取每个服务的配置
            for global_name, service_entity in all_services.items():
                # 提取服务原始名称（用于 mcpServers 的 key）
                service_name = service_entity.get("service_original_name")
                if not service_name:
                    # 尝试从 global_name 解析
                    # global_name 格式: service_name@agent_id
                    if "@" in global_name:
                        service_name = global_name.split("@")[0]
                    else:
                        service_name = global_name
                
                # 提取服务配置
                config = self._logic_core.extract_service_config(service_entity)
                
                # 提取 source_agent
                source_agent = service_entity.get("source_agent", "global_agent_store")
                
                if config:
                    services_data[service_name] = {
                        "config": config,
                        "source_agent": source_agent
                    }
            
            logger.debug(f"[SHOW_CONFIG_SHELL] 提取到 {len(services_data)} 个服务配置")
            
            return services_data
            
        except Exception as e:
            logger.error(f"[SHOW_CONFIG_SHELL] 读取所有服务数据失败: {e}")
            raise
    
    async def _read_agent_services_data_async(
        self,
        agent_id: str
    ) -> Dict[str, Dict[str, Any]]:
        """
        从 pykv 异步读取指定 Agent 的服务数据
        
        Args:
            agent_id: Agent ID
        
        Returns:
            该 Agent 的服务配置数据
            格式: {service_original_name: {"config": {...}}}
        """
        services_data = {}
        
        try:
            # 从 pykv 实体层读取所有服务实体
            all_services = await self._cache_layer.get_all_entities_async("services")
            
            # 过滤属于指定 agent_id 的服务
            for global_name, service_entity in all_services.items():
                # 获取服务所属的 agent_id
                entity_agent_id = service_entity.get("source_agent")
                if not entity_agent_id:
                    # 尝试从 global_name 解析
                    # global_name 格式: service_name@agent_id
                    if "@" in global_name:
                        _, entity_agent_id = global_name.rsplit("@", 1)
                    else:
                        entity_agent_id = "global_agent_store"
                
                if entity_agent_id == agent_id:
                    # 提取服务原始名称
                    service_name = service_entity.get("service_original_name")
                    if not service_name:
                        if "@" in global_name:
                            service_name = global_name.split("@")[0]
                        else:
                            service_name = global_name
                    
                    # 提取服务配置
                    config = self._logic_core.extract_service_config(service_entity)
                    
                    if config:
                        services_data[service_name] = {"config": config}
            
            logger.debug(
                f"[SHOW_CONFIG_SHELL] Agent {agent_id} 的服务数据: "
                f"{len(services_data)} 个服务"
            )
            
            return services_data
            
        except Exception as e:
            logger.error(f"[SHOW_CONFIG_SHELL] 读取 Agent {agent_id} 服务数据失败: {e}")
            raise
    
    async def _check_agent_exists_async(self, agent_id: str) -> bool:
        """
        从 pykv 异步检查 Agent 是否存在
        
        通过检查是否有属于该 Agent 的服务来判断 Agent 是否存在。
        
        Args:
            agent_id: Agent ID
        
        Returns:
            Agent 是否存在
        """
        try:
            # 方法1: 检查 Agent 实体是否存在
            agent_entity = await self._cache_layer.get_entity("agents", agent_id)
            if agent_entity:
                return True
            
            # 方法2: 检查是否有属于该 Agent 的服务
            all_services = await self._cache_layer.get_all_entities_async("services")
            for global_name, service_entity in all_services.items():
                entity_agent_id = service_entity.get("source_agent")
                if not entity_agent_id and "@" in global_name:
                    _, entity_agent_id = global_name.rsplit("@", 1)
                
                if entity_agent_id == agent_id:
                    return True
            
            # 方法3: 特殊处理 global_agent_store
            if agent_id == "global_agent_store":
                return True
            
            return False
            
        except Exception as e:
            logger.error(f"[SHOW_CONFIG_SHELL] 检查 Agent {agent_id} 是否存在失败: {e}")
            raise
