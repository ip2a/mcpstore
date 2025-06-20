"""
MCPStore Context Module
提供 MCPStore 的上下文管理功能
"""

from typing import Dict, List, Optional, Any, Union, TYPE_CHECKING
from enum import Enum
from mcpstore.core.models.tool import ToolExecutionRequest, ToolInfo
from mcpstore.core.models.common import ExecutionResponse
from mcpstore.core.models.service import (
    ServiceInfo, AddServiceRequest, ServiceConfigUnion,
    URLServiceConfig, CommandServiceConfig, MCPServerConfig
)
import logging
from .exceptions import ServiceNotFoundError, InvalidConfigError, DeleteServiceError
from .async_sync_helper import get_global_helper

if TYPE_CHECKING:
    from ..adapters.langchain_adapter import LangChainAdapter
    from .unified_config import UnifiedConfigManager

class ContextType(Enum):
    """上下文类型"""
    STORE = "store"
    AGENT = "agent"

class MCPStoreContext:
    """
    MCPStore上下文类
    负责处理具体的业务操作，维护操作的上下文环境
    """
    def __init__(self, store: 'MCPStore', agent_id: Optional[str] = None):
        self._store = store
        self._agent_id = agent_id
        self._context_type = ContextType.STORE if agent_id is None else ContextType.AGENT

        # 异步/同步兼容助手
        self._sync_helper = get_global_helper()

        # 扩展预留
        self._metadata: Dict[str, Any] = {}
        self._config: Dict[str, Any] = {}
        self._cache: Dict[str, Any] = {}

    def for_langchain(self) -> 'LangChainAdapter':
        """返回一个 LangChain 适配器实例，用于后续的 LangChain 相关操作。"""
        from ..langchain_adapter import LangChainAdapter
        return LangChainAdapter(self)

    # === 核心服务接口 ===
    def list_services(self) -> List[ServiceInfo]:
        """
        列出服务列表（同步版本）
        - store上下文：聚合 main_client 下所有 client_id 的服务
        - agent上下文：聚合 agent_id 下所有 client_id 的服务
        """
        return self._sync_helper.run_async(self.list_services_async())

    async def list_services_async(self) -> List[ServiceInfo]:
        """
        列出服务列表（异步版本）
        - store上下文：聚合 main_client 下所有 client_id 的服务
        - agent上下文：聚合 agent_id 下所有 client_id 的服务
        """
        if self._context_type == ContextType.STORE:
            return await self._store.list_services()
        else:
            return await self._store.list_services(self._agent_id, agent_mode=True)

    def add_service(self, config: Union[ServiceConfigUnion, List[str], None] = None, json_file: str = None) -> 'MCPStoreContext':
        """
        增强版的服务添加方法（同步版本），支持多种配置格式

        Args:
            config: 服务配置，支持多种格式
            json_file: JSON文件路径，如果指定则读取该文件作为配置
        """
        return self._sync_helper.run_async(self.add_service_async(config, json_file))

    async def add_service_async(self, config: Union[ServiceConfigUnion, List[str], None] = None, json_file: str = None) -> 'MCPStoreContext':
        """
        增强版的服务添加方法，支持多种配置格式：
        1. URL方式：
           await add_service({
               "name": "weather",
               "url": "https://weather-api.example.com/mcp",
               "transport": "streamable-http"
           })

        2. 本地命令方式：
           await add_service({
               "name": "assistant",
               "command": "python",
               "args": ["./assistant_server.py"],
               "env": {"DEBUG": "true"}
           })

        3. MCPConfig字典方式：
           await add_service({
               "mcpServers": {
                   "weather": {
                       "url": "https://weather-api.example.com/mcp"
                   }
               }
           })

        4. 服务名称列表方式（从现有配置中选择）：
           await add_service(['weather', 'assistant'])

        5. 无参数方式（仅限Store上下文）：
           await add_service()  # 注册所有服务

        6. JSON文件方式：
           await add_service(json_file="path/to/config.json")  # 读取JSON文件作为配置

        所有新添加的服务都会同步到 mcp.json 配置文件中。

        Args:
            config: 服务配置，支持多种格式
            json_file: JSON文件路径，如果指定则读取该文件作为配置

        Returns:
            MCPStoreContext: 返回自身实例以支持链式调用
        """
        try:
            # 处理json_file参数
            if json_file is not None:
                print(f"[INFO][add_service] 从JSON文件读取配置: {json_file}")
                try:
                    import json
                    import os

                    if not os.path.exists(json_file):
                        raise Exception(f"JSON文件不存在: {json_file}")

                    with open(json_file, 'r', encoding='utf-8') as f:
                        file_config = json.load(f)

                    print(f"[INFO][add_service] 成功读取JSON文件，配置: {file_config}")

                    # 如果同时指定了config和json_file，优先使用json_file
                    if config is not None:
                        print("[WARN][add_service] 同时指定了config和json_file参数，将使用json_file")

                    config = file_config

                except Exception as e:
                    raise Exception(f"读取JSON文件失败: {e}")

            # 如果既没有config也没有json_file，且不是Store模式的全量注册，则报错
            if config is None and json_file is None and self._context_type != ContextType.STORE:
                raise Exception("必须指定config参数或json_file参数")

        except Exception as e:
            print(f"[ERROR][add_service] 参数处理失败: {e}")
            raise

        try:
            # 获取正确的 agent_id（Store级别使用main_client作为agent_id）
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.orchestrator.client_manager.main_client_id
            print(f"[INFO][add_service] 当前模式: {self._context_type.name}, agent_id: {agent_id}")
            
            # 处理不同的输入格式
            if config is None:
                # Store模式下的全量注册
                if self._context_type == ContextType.STORE:
                    print("[INFO][add_service] STORE模式-全量注册所有服务")
                    resp = await self._store.register_json_service()
                    print(f"[INFO][add_service] 注册结果: {resp}")
                    if not (resp and resp.service_names):
                        raise Exception("服务注册失败")
                else:
                    print("[WARN][add_service] AGENT模式-未指定服务配置")
                    raise Exception("AGENT模式必须指定服务配置")
                    
            # 处理服务名称列表
            elif isinstance(config, list):
                if not config:
                    raise Exception("服务名称列表为空")
                    
                print(f"[INFO][add_service] 注册指定服务: {config}")
                resp = await self._store.register_json_service(
                    client_id=agent_id,
                    service_names=config
                )
                print(f"[INFO][add_service] 注册结果: {resp}")
                if not (resp and resp.service_names):
                    raise Exception("服务注册失败")
                
            # 处理字典格式的配置
            elif isinstance(config, dict):
                # 转换为标准格式
                if "mcpServers" in config:
                    # 已经是MCPConfig格式
                    mcp_config = config
                else:
                    # 单个服务配置，需要转换为MCPConfig格式
                    service_name = config.get("name")
                    if not service_name:
                        raise Exception("服务配置缺少name字段")
                        
                    mcp_config = {
                        "mcpServers": {
                            service_name: {k: v for k, v in config.items() if k != "name"}
                        }
                    }
                
                # 更新配置文件和处理同名服务
                try:
                    # 1. 加载现有配置
                    current_config = self._store.config.load_config()

                    # 2. 合并新配置到mcp.json
                    for name, service_config in mcp_config["mcpServers"].items():
                        current_config["mcpServers"][name] = service_config

                    # 3. 保存更新后的配置
                    self._store.config.save_config(current_config)

                    # 4. 重新加载配置以确保同步
                    self._store.config.load_config()

                    # 5. 处理同名服务替换（新增逻辑）
                    created_client_ids = []
                    for name, service_config in mcp_config["mcpServers"].items():
                        # 使用新的同名服务处理逻辑
                        success = self._store.client_manager.replace_service_in_agent(
                            agent_id=agent_id,
                            service_name=name,
                            new_service_config=service_config
                        )
                        if not success:
                            raise Exception(f"替换服务 {name} 失败")
                        print(f"[INFO][add_service] 成功处理同名服务: {name}")

                        # 获取刚创建的client_id用于Registry注册
                        client_ids = self._store.client_manager.get_agent_clients(agent_id)
                        for client_id in client_ids:
                            client_config = self._store.client_manager.get_client_config(client_id)
                            if client_config and name in client_config.get("mcpServers", {}):
                                if client_id not in created_client_ids:
                                    created_client_ids.append(client_id)
                                break

                    # 6. 注册服务到Registry（使用已创建的client配置）
                    print(f"[INFO][add_service] 注册服务到Registry，使用client_ids: {created_client_ids}")
                    for client_id in created_client_ids:
                        client_config = self._store.client_manager.get_client_config(client_id)
                        if client_config:
                            try:
                                await self._store.orchestrator.register_json_services(client_config, client_id=client_id)
                                print(f"[INFO][add_service] 成功注册client {client_id} 到Registry")
                            except Exception as e:
                                print(f"[WARN][add_service] 注册client {client_id} 到Registry失败: {e}")

                    print(f"[INFO][add_service] 服务配置更新和Registry注册完成")

                except Exception as e:
                    raise Exception(f"更新配置文件失败: {e}")
            
            else:
                raise Exception(f"不支持的配置格式: {type(config)}")
            
            return self
            
        except Exception as e:
            print(f"[ERROR][add_service] 服务添加失败: {e}")
            raise

    def list_tools(self) -> List[ToolInfo]:
        """
        列出工具列表（同步版本）
        - store上下文：聚合 main_client 下所有 client_id 的工具
        - agent上下文：聚合 agent_id 下所有 client_id 的工具
        """
        return self._sync_helper.run_async(self.list_tools_async())

    async def list_tools_async(self) -> List[ToolInfo]:
        """
        列出工具列表（异步版本）
        - store上下文：聚合 main_client 下所有 client_id 的工具
        - agent上下文：聚合 agent_id 下所有 client_id 的工具
        """
        if self._context_type == ContextType.STORE:
            return await self._store.list_tools()
        else:
            return await self._store.list_tools(self._agent_id, agent_mode=True)

    def check_services(self) -> dict:
        """
        健康检查（同步版本），store/agent上下文自动判断
        - store上下文：聚合 main_client 下所有 client_id 的服务健康状态
        - agent上下文：聚合 agent_id 下所有 client_id 的服务健康状态
        """
        return self._sync_helper.run_async(self.check_services_async())

    async def check_services_async(self) -> dict:
        """
        异步健康检查，store/agent上下文自动判断
        - store上下文：聚合 main_client 下所有 client_id 的服务健康状态
        - agent上下文：聚合 agent_id 下所有 client_id 的服务健康状态
        """
        if self._context_type.name == 'STORE':
            return await self._store.get_health_status()
        elif self._context_type.name == 'AGENT':
            return await self._store.get_health_status(self._agent_id, agent_mode=True)
        else:
            print(f"[ERROR][check_services] 未知上下文类型: {self._context_type}")
            return {}

    def get_service_info(self, name: str) -> Any:
        """
        获取服务详情（同步版本），支持 store/agent 上下文
        - store上下文：在 main_client 下的所有 client 中查找服务
        - agent上下文：在指定 agent_id 下的所有 client 中查找服务
        """
        return self._sync_helper.run_async(self.get_service_info_async(name))

    async def get_service_info_async(self, name: str) -> Any:
        """
        获取服务详情（异步版本），支持 store/agent 上下文
        - store上下文：在 main_client 下的所有 client 中查找服务
        - agent上下文：在指定 agent_id 下的所有 client 中查找服务
        """
        if not name:
            return {}

        if self._context_type == ContextType.STORE:
            print(f"[INFO][get_service_info] STORE模式-在main_client中查找服务: {name}")
            return await self._store.get_service_info(name)
        elif self._context_type == ContextType.AGENT:
            print(f"[INFO][get_service_info] AGENT模式-在agent({self._agent_id})中查找服务: {name}")
            return await self._store.get_service_info(name, self._agent_id)
        else:
            print(f"[ERROR][get_service_info] 未知上下文类型: {self._context_type}")
            return {}

    def use_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        使用工具（同步版本），支持 store/agent 上下文
        - store上下文：在 main_client 下的所有 client 中查找并使用工具
        - agent上下文：在指定 agent_id 下的所有 client 中查找并使用工具

        Args:
            tool_name: 工具名称，格式为 service_toolname
            args: 工具参数

        Returns:
            Any: 工具执行结果
        """
        return self._sync_helper.run_async(self.use_tool_async(tool_name, args))

    async def use_tool_async(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        使用工具（异步版本），支持 store/agent 上下文
        - store上下文：在 main_client 下的所有 client 中查找并使用工具
        - agent上下文：在指定 agent_id 下的所有 client 中查找并使用工具

        Args:
            tool_name: 工具名称，格式为 service_toolname
            args: 工具参数

        Returns:
            Any: 工具执行结果
        """
        # 从工具名称中提取服务名称
        if "_" not in tool_name:
            raise ValueError(f"Invalid tool name format: {tool_name}. Expected format: service_toolname")

        if self._context_type == ContextType.STORE:
            print(f"[INFO][use_tool] STORE模式-在main_client中使用工具: {tool_name}")
            request = ToolExecutionRequest(
                tool_name=tool_name,
                args=args
            )
        else:
            print(f"[INFO][use_tool] AGENT模式-在agent({self._agent_id})中使用工具: {tool_name}")
            request = ToolExecutionRequest(
                tool_name=tool_name,
                args=args,
                agent_id=self._agent_id
            )

        return await self._store.process_tool_request(request)

    # === 上下文信息 ===
    @property
    def context_type(self) -> ContextType:
        """获取上下文类型"""
        return self._context_type

    @property
    def agent_id(self) -> Optional[str]:
        """获取当前agent_id"""
        return self._agent_id 

    def show_mcpconfig(self) -> Dict[str, Any]:
        """
        根据当前上下文（store/agent）获取对应的配置信息
        
        Returns:
            Dict[str, Any]: 包含所有相关client配置的字典
        """
        # 获取所有相关的client_ids
        agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.orchestrator.client_manager.main_client_id
        client_ids = self._store.orchestrator.client_manager.get_agent_clients(agent_id)
        
        # 获取每个client的配置
        result = {}
        for client_id in client_ids:
            client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
            if client_config:
                result[client_id] = client_config
                
        return result 

    def update_service(self, name: str, config: Dict[str, Any]) -> bool:
        """
        更新服务配置（同步版本）

        Args:
            name: 服务名称（不可更改）
            config: 新的服务配置

        Returns:
            bool: 更新是否成功
        """
        return self._sync_helper.run_async(self.update_service_async(name, config))

    async def update_service_async(self, name: str, config: Dict[str, Any]) -> bool:
        """
        更新服务配置
        
        Args:
            name: 服务名称（不可更改）
            config: 新的服务配置
            
        Returns:
            bool: 更新是否成功
            
        Raises:
            ServiceNotFoundError: 服务不存在
            InvalidConfigError: 配置无效
        """
        try:
            # 1. 验证服务是否存在
            if not self._store.config.get_service_config(name):
                raise ServiceNotFoundError(f"Service {name} not found")
            
            # 2. 更新 mcp.json 中的配置（无论是 store 还是 agent 级别都要更新）
            if not self._store.config.update_service(name, config):
                raise InvalidConfigError(f"Failed to update service {name}")
            
            # 3. 获取需要更新的 client_ids
            if self._context_type == ContextType.STORE:
                # store 级别：更新所有 client
                client_ids = self._store.orchestrator.client_manager.get_main_client_ids()
            else:
                # agent 级别：同样更新所有配置
                client_ids = self._store.orchestrator.client_manager.get_main_client_ids()
            
            # 4. 更新每个 client 的配置
            for client_id in client_ids:
                client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
                if client_config and name in client_config.get("mcpServers", {}):
                    client_config["mcpServers"][name] = config
                    self._store.orchestrator.client_manager.save_client_config(client_id, client_config)
            
            return True
            
        except Exception as e:
            logging.error(f"Failed to update service {name}: {str(e)}")
            raise

    def delete_service(self, name: str) -> bool:
        """
        删除服务（同步版本）

        Args:
            name: 要删除的服务名称

        Returns:
            bool: 删除是否成功
        """
        return self._sync_helper.run_async(self.delete_service_async(name))

    async def delete_service_async(self, name: str) -> bool:
        """
        删除服务
        
        Args:
            name: 要删除的服务名称
            
        Returns:
            bool: 删除是否成功
            
        Raises:
            ServiceNotFoundError: 服务不存在
            DeleteServiceError: 删除失败
        """
        try:
            # 1. 验证服务是否存在
            if not self._store.config.get_service_config(name):
                raise ServiceNotFoundError(f"Service {name} not found")
            
            # 2. 根据上下文确定删除范围
            if self._context_type == ContextType.STORE:
                # store 级别：删除所有 client 中的服务并更新 mcp.json
                client_ids = self._store.orchestrator.client_manager.get_main_client_ids()
                
                # 从 mcp.json 中删除
                if not self._store.config.remove_service(name):
                    raise DeleteServiceError(f"Failed to remove service {name} from mcp.json")
                
                # 从所有 client 配置中删除
                for client_id in client_ids:
                    client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
                    if client_config and name in client_config.get("mcpServers", {}):
                        del client_config["mcpServers"][name]
                        self._store.orchestrator.client_manager.save_client_config(client_id, client_config)
                
            else:
                # agent 级别：只删除该 agent 的 client 列表中的服务
                client_ids = self._store.orchestrator.client_manager.get_agent_clients(self._agent_id)
                
                # 从指定 agent 的 client 配置中删除
                for client_id in client_ids:
                    client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
                    if client_config and name in client_config.get("mcpServers", {}):
                        del client_config["mcpServers"][name]
                        self._store.orchestrator.client_manager.save_client_config(client_id, client_config)
            
            return True
            
        except Exception as e:
            logging.error(f"Failed to delete service {name}: {str(e)}")
            raise

    def for_langchain(self) -> 'LangChainAdapter':
        """返回LangChain适配器实例"""
        from mcpstore.adapters.langchain_adapter import LangChainAdapter
        return LangChainAdapter(self)

    async def reset_config(self) -> bool:
        """
        重置配置
        - Store级别：重置main_client的所有配置
        - Agent级别：重置指定Agent的所有配置和映射

        Returns:
            是否成功重置
        """
        try:
            if self._agent_id is None:
                # Store级别重置 - 使用main_client作为agent_id
                main_client_id = self._store.orchestrator.client_manager.main_client_id
                success = self._store.orchestrator.client_manager.reset_agent_config(main_client_id)
                if success:
                    # 清理registry中的store级别数据
                    if main_client_id in self._store.orchestrator.registry.sessions:
                        del self._store.orchestrator.registry.sessions[main_client_id]
                    if main_client_id in self._store.orchestrator.registry.service_health:
                        del self._store.orchestrator.registry.service_health[main_client_id]
                    if main_client_id in self._store.orchestrator.registry.tool_cache:
                        del self._store.orchestrator.registry.tool_cache[main_client_id]
                    if main_client_id in self._store.orchestrator.registry.tool_to_session_map:
                        del self._store.orchestrator.registry.tool_to_session_map[main_client_id]

                    # 清理重连队列中与该client相关的条目
                    self._cleanup_reconnection_queue_for_client(main_client_id)

                    logging.info("Successfully reset store config and registry")
                return success
            else:
                # Agent级别重置
                success = self._store.orchestrator.client_manager.reset_agent_config(self._agent_id)
                if success:
                    # 清理registry中的agent级别数据
                    if self._agent_id in self._store.orchestrator.registry.sessions:
                        del self._store.orchestrator.registry.sessions[self._agent_id]
                    if self._agent_id in self._store.orchestrator.registry.service_health:
                        del self._store.orchestrator.registry.service_health[self._agent_id]
                    if self._agent_id in self._store.orchestrator.registry.tool_cache:
                        del self._store.orchestrator.registry.tool_cache[self._agent_id]
                    if self._agent_id in self._store.orchestrator.registry.tool_to_session_map:
                        del self._store.orchestrator.registry.tool_to_session_map[self._agent_id]

                    # 清理重连队列中与该agent相关的条目
                    agent_clients = self._store.orchestrator.client_manager.get_agent_clients(self._agent_id)
                    for client_id in agent_clients:
                        self._cleanup_reconnection_queue_for_client(client_id)

                    logging.info(f"Successfully reset agent {self._agent_id} config and registry")
                return success

        except Exception as e:
            logging.error(f"Failed to reset config: {str(e)}")
            return False

    def _cleanup_reconnection_queue_for_client(self, client_id: str):
        """清理重连队列中与指定client相关的条目"""
        try:
            # 查找所有与该client相关的重连条目
            entries_to_remove = []
            for service_key in self._store.orchestrator.smart_reconnection.entries:
                if service_key.startswith(f"{client_id}:"):
                    entries_to_remove.append(service_key)

            # 移除这些条目
            for entry in entries_to_remove:
                self._store.orchestrator.smart_reconnection.remove_service(entry)

            if entries_to_remove:
                logging.info(f"Cleaned up {len(entries_to_remove)} reconnection queue entries for client {client_id}")

        except Exception as e:
            logging.warning(f"Failed to cleanup reconnection queue for client {client_id}: {e}")

    def show_mcpconfig(self) -> dict:
        """显示MCP配置"""
        try:
            config = self._store.config.load_config()
            # 确保返回格式正确
            if isinstance(config, dict) and 'mcpServers' in config:
                return config
            else:
                logging.warning("Invalid MCP config format")
                return {"mcpServers": {}}
        except Exception as e:
            logging.error(f"Failed to show MCP config: {e}")
            return {"mcpServers": {}}

    async def get_service_status(self, name: str) -> dict:
        """获取单个服务的状态信息"""
        try:
            service_info = await self.get_service_info(name)
            if hasattr(service_info, 'service') and service_info.service:
                return {
                    "name": service_info.service.name,
                    "status": service_info.service.status,
                    "connected": service_info.connected,
                    "tool_count": service_info.service.tool_count,
                    "last_heartbeat": service_info.service.last_heartbeat,
                    "transport_type": service_info.service.transport_type
                }
            else:
                return {
                    "name": name,
                    "status": "not_found",
                    "connected": False,
                    "tool_count": 0,
                    "last_heartbeat": None,
                    "transport_type": None
                }
        except Exception as e:
            logging.error(f"Failed to get service status for {name}: {e}")
            return {
                "name": name,
                "status": "error",
                "connected": False,
                "error": str(e)
            }

    async def restart_service(self, name: str) -> bool:
        """重启指定服务"""
        try:
            # 首先验证服务是否存在
            service_info = await self.get_service_info(name)
            if not (hasattr(service_info, 'service') and service_info.service):
                logging.error(f"Service {name} not found in registry")
                return False

            # 获取服务配置
            service_config = self._store.config.get_service_config(name)
            if not service_config:
                logging.error(f"Service config not found for {name} in mcp.json")
                # 尝试从当前运行的服务中获取配置信息
                logging.info(f"Attempting to restart service {name} without config reload")
                # 简单的重连尝试
                try:
                    # 获取当前上下文的client_id
                    agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.orchestrator.client_manager.main_client_id
                    client_ids = self._store.orchestrator.client_manager.get_agent_clients(agent_id)

                    for client_id in client_ids:
                        if self._store.orchestrator.registry.has_service(client_id, name):
                            # 尝试重新连接服务
                            success, message = await self._store.orchestrator.connect_service(name)
                            if success:
                                logging.info(f"Service {name} reconnected successfully")
                                return True

                    logging.error(f"Failed to reconnect service {name}")
                    return False
                except Exception as e:
                    logging.error(f"Failed to reconnect service {name}: {e}")
                    return False

            # 先删除服务
            delete_success = await self.delete_service(name)
            if not delete_success:
                logging.warning(f"Failed to delete service {name} during restart, attempting to continue")

            # 等待一小段时间确保服务完全停止
            import asyncio
            await asyncio.sleep(1)

            # 构造添加服务的配置
            add_config = {
                "name": name,
                **service_config
            }

            # 重新添加服务
            await self.add_service(add_config)
            logging.info(f"Service {name} restarted successfully")
            return True

        except Exception as e:
            logging.error(f"Failed to restart service {name}: {e}")
            return False

    async def update_service(self, name: str, config: dict) -> bool:
        """更新服务配置"""
        try:
            # 验证服务是否存在
            service_info = await self.get_service_info(name)
            if not (hasattr(service_info, 'service') and service_info.service):
                logging.error(f"Service {name} not found")
                return False

            # 更新配置文件
            current_config = self._store.config.get_service_config(name) or {}
            updated_config = {**current_config, **config}

            # 移除name字段（如果存在）因为它是key
            if 'name' in updated_config:
                del updated_config['name']

            # 更新到配置文件
            success = self._store.config.update_service_config(name, updated_config)
            if not success:
                logging.error(f"Failed to update config for service {name}")
                return False

            # 重启服务以应用新配置
            restart_success = await self.restart_service(name)
            if restart_success:
                logging.info(f"Service {name} updated and restarted successfully")
                return True
            else:
                logging.warning(f"Service {name} config updated but restart failed")
                return False

        except Exception as e:
            logging.error(f"Failed to update service {name}: {e}")
            return False

    async def reset_json_config(self) -> bool:
        """
        重置JSON配置文件（仅Store级别可用）
        将mcp.json备份后重置为空字典

        Returns:
            是否成功重置
        """
        if self._agent_id is not None:
            logging.warning("reset_json_config is only available for store level")
            return False

        try:
            success = self._store.config.reset_json_config()
            if success:
                # 重置后需要重新加载配置
                await self._store.orchestrator.setup()
                logging.info("Successfully reset JSON config and reloaded")
            return success

        except Exception as e:
            logging.error(f"Failed to reset JSON config: {str(e)}")
            return False

    async def restore_default_config(self) -> bool:
        """
        恢复默认配置（仅Store级别可用）
        恢复高德和天气服务的默认配置

        Returns:
            是否成功恢复
        """
        if self._agent_id is not None:
            logging.warning("restore_default_config is only available for store level")
            return False

        try:
            success = self._store.config.restore_default_config()
            if success:
                # 恢复后需要重新加载配置
                await self._store.orchestrator.setup()
                logging.info("Successfully restored default config and reloaded")
            return success

        except Exception as e:
            logging.error(f"Failed to restore default config: {str(e)}")
            return False

    def get_unified_config(self) -> 'UnifiedConfigManager':
        """获取统一配置管理器

        Returns:
            UnifiedConfigManager: 统一配置管理器实例
        """
        return self._store.get_unified_config()
