"""
MCPStore Service Operations Module - Event-Driven Architecture
Implementation of service-related operations using event-driven pattern
"""

import asyncio
import logging
from typing import Dict, List, Optional, Any, Union, Tuple

from mcpstore.core.models.service import ServiceInfo, ServiceConfigUnion, ServiceConnectionState
from .types import ContextType

logger = logging.getLogger(__name__)


class AddServiceWaitStrategy:
    """添加服务等待策略"""

    def __init__(self):
        # 不同服务类型的默认等待时间（毫秒）
        self.default_timeouts = {
            'remote': 2000,  # 远程服务2秒
            'local': 4000,   # 本地服务4秒
        }

    def parse_wait_parameter(self, wait_param: Union[str, int, float]) -> Optional[float]:
        """
        解析等待参数

        Args:
            wait_param: 等待参数，支持:
                - "auto": 自动根据服务类型判断
                - 数字: 毫秒数
                - 字符串数字: 毫秒数

        Returns:
            float: 等待时间（秒），None表示需要自动判断
        """
        if wait_param == "auto":
            return None  # 表示需要自动判断

        # 尝试解析为数字（毫秒）
        try:
            if isinstance(wait_param, str):
                ms = float(wait_param)
            else:
                ms = float(wait_param)

            # 转换为秒，最小100ms，最大30秒
            seconds = max(0.1, min(30.0, ms / 1000.0))
            return seconds

        except (ValueError, TypeError):
            logger.warning(f"Invalid wait parameter '{wait_param}', using auto mode")
            return None

    def get_service_wait_timeout(self, service_config: Dict[str, Any]) -> float:
        """
        根据服务配置获取等待超时时间

        Args:
            service_config: 服务配置

        Returns:
            float: 等待时间（秒）
        """
        if self._is_remote_service(service_config):
            return self.default_timeouts['remote'] / 1000.0  # 转换为秒
        else:
            return self.default_timeouts['local'] / 1000.0   # 转换为秒

    def _is_remote_service(self, service_config: Dict[str, Any]) -> bool:
        """判断是否为远程服务"""
        return bool(service_config.get('url'))

    def get_max_wait_timeout(self, services_config: Dict[str, Dict[str, Any]]) -> float:
        """
        获取多个服务的最大等待时间

        Args:
            services_config: 服务配置字典

        Returns:
            float: 最大等待时间（秒）
        """
        if not services_config:
            return 2.0  # 默认2秒

        max_timeout = 0.0
        for service_config in services_config.values():
            timeout = self.get_service_wait_timeout(service_config)
            max_timeout = max(max_timeout, timeout)

        return max_timeout


class ServiceOperationsMixin:
    """
    Service operations mixin class - Event-Driven Architecture

    职责：提供用户API，委托给应用服务
    """

    @staticmethod
    def _find_mcp_servers_key(config: Dict[str, Any]) -> Optional[str]:
        """
        查找 mcpServers 键（不区分大小写）
        
        Args:
            config: 配置字典
            
        Returns:
            Optional[str]: 找到的键名（原始大小写），如果没找到返回 None
        """
        if not isinstance(config, dict):
            return None
        
        for key in config.keys():
            if key.lower() == "mcpservers":
                return key
        return None
    
    @staticmethod
    def _normalize_mcp_servers(config: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """
        标准化 mcpServers 配置（将键名统一为 "mcpServers"）
        
        Args:
            config: 配置字典
            
        Returns:
            Optional[Dict[str, Any]]: 标准化后的配置，如果没有 mcpServers 键返回 None
        """
        key = ServiceOperationsMixin._find_mcp_servers_key(config)
        if not key:
            return None
        
        # 如果已经是标准格式，直接返回
        if key == "mcpServers":
            return config
        
        # 标准化为 mcpServers
        standardized = {k: v for k, v in config.items() if k != key}
        standardized["mcpServers"] = config[key]
        return standardized

    # === Core service interface ===
    def list_services(self) -> List[ServiceInfo]:
        """
        List services (synchronous wrapper) - 始终桥接到异步实现
        """
        try:
            return self._run_async_via_bridge(
                self.list_services_async(),
                op_name="service_operations.list_services"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] list_services 失败: {e}")
            return []

    async def list_services_async(self) -> List[ServiceInfo]:
        """
        List services (asynchronous version)
        - store context: aggregate services from all client_ids under global_agent_store
        - agent context: show only agent's services with local names (transparent proxy)
        """
        if self._context_type == ContextType.STORE:
            result = await self._store.list_services()
            try:
                logger.info(f"[LIST_SERVICES] context=STORE count={len(result)}")
            except Exception:
                pass
            return result
        else:
            # Agent mode: 透明代理 - 只显示属于该 Agent 的服务，使用本地名称
            result = await self._get_agent_service_view()
            try:
                logger.info(f"[LIST_SERVICES] context=AGENT agent_id={self._agent_id} count={len(result)}")
            except Exception:
                pass
            return result

    def add_service(self,
                     config: Union[ServiceConfigUnion, Dict[str, Any], str, None] = None,
                     json_file: str = None,
                     auth: Optional[str] = None,
                     token: Optional[str] = None,
                     api_key: Optional[str] = None,
                     headers: Optional[Dict[str, str]] = None) -> 'MCPStoreContext':
        """
        添加服务（同步入口，使用新架构避免死锁）。

        - 使用Functional Core, Imperative Shell架构
        - 完全避免_sync_helper.run_async和_sync_to_kv调用
        - 接受：单服务配置字典/JSON字符串/包含 mcpServers 的字典
        - 认证：token/api_key 会标准化为 headers 并仅以 headers 落盘
        - 等待：不等待连接；请使用 wait_service(...) 单独控制
        """
        # 标准化认证（token/api_key/auth -> headers）
        final_config = self._apply_auth_to_config(config, auth, token, api_key, headers)

        # 处理json_file参数（可选）
        if json_file is not None:
            logger.info(f"从JSON文件读取配置: {json_file}")
            try:
                import json
                import os

                if not os.path.exists(json_file):
                    raise Exception(f"JSON文件不存在: {json_file}")

                with open(json_file, 'r', encoding='utf-8') as f:
                    file_config = json.load(f)

                logger.info(f"成功读取JSON文件，配置: {file_config}")

                # 如果同时指定了config和json_file，优先使用json_file
                if final_config is not None:
                    logger.warning("同时指定了config和json_file参数，将使用json_file")

                final_config = file_config

            except Exception as e:
                raise Exception(f"读取JSON文件失败: {e}")

        # 支持 config 传入 JSON 字符串（单服务或 mcpServers/root 映射）
        if isinstance(final_config, str):
            try:
                import json as _json
                cfg = _json.loads(final_config)
                final_config = cfg
            except Exception:
                raise Exception("config 为字符串时必须是合法的 JSON")

        # 使用新架构：同步外壳
            if not hasattr(self, '_service_management_sync_shell'):
                from ..architecture import ServiceManagementFactory
                self._service_management_sync_shell, _, _ = ServiceManagementFactory.create_service_management(
                    self._store.registry,
                    self._store.orchestrator,
                    agent_id=self._agent_id or self._store.client_manager.global_agent_store_id
                )

        # 直接调用同步外壳，完全避免_sync_helper.run_async
        result = self._service_management_sync_shell.add_service(final_config)

        logger.debug(f"[NEW_ARCH] add_service 结果: {result.get('success', False)}")
        return self

    def add_service_with_details(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> Dict[str, Any]:
        """
        添加服务并返回详细信息（同步版本）

        Args:
            config: 服务配置

        Returns:
            Dict: 包含添加结果的详细信息
        """
        try:
            return self._run_async_via_bridge(
                self.add_service_with_details_async(config),
                op_name="service_operations.add_service_with_details"
            )
        except Exception as e:
            logger.error(f"[NEW_ARCH] add_service_with_details 失败: {e}")
            return {
                "success": False,
                "added_services": [],
                "failed_services": self._extract_service_names(config),
                "service_details": {},
                "total_services": 0,
                "total_tools": 0,
                "message": str(e)
            }

    async def add_service_with_details_async(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> Dict[str, Any]:
        """
        添加服务并返回详细信息（异步版本）

        Args:
            config: 服务配置

        Returns:
            Dict: 包含添加结果的详细信息
        """
        logger.debug(f"Adding service with config: {type(config).__name__}")

        # 预处理配置
        try:
            processed_config = self._preprocess_service_config(config)
            logger.debug(f"Config preprocessed successfully")
        except ValueError as e:
            logger.error(f"Config preprocessing failed: {e}")
            return {
                "success": False,
                "added_services": [],
                "failed_services": self._extract_service_names(config),
                "service_details": {},
                "total_services": 0,
                "total_tools": 0,
                "message": str(e)
            }

        # 添加服务
        try:
            logger.debug("Calling add_service_async")
            result = await self.add_service_async(processed_config)
            logger.debug(f"Service addition result: {result is not None}")
        except Exception as e:
            logger.error(f"Service addition failed: {e}")
            return {
                "success": False,
                "added_services": [],
                "failed_services": self._extract_service_names(config),
                "service_details": {},
                "total_services": 0,
                "total_tools": 0,
                "message": f"Service addition failed: {str(e)}"
            }

        if result is None:
            logger.error("Service addition returned None")
            return {
                "success": False,
                "added_services": [],
                "failed_services": self._extract_service_names(config),
                "service_details": {},
                "total_services": 0,
                "total_tools": 0,
                "message": "Service addition failed"
            }

        # 获取添加后的详情
        logger.debug("Retrieving updated services and tools list")
        services = await self.list_services_async()
        tools = await self.list_tools_async()
        logger.debug(f"Current services: {len(services)}, tools: {len(tools)}")
        logger.debug(f"Service names: {[getattr(s, 'name', 'unknown') for s in services]}")

        # 分析添加结果
        expected_service_names = self._extract_service_names(config)
        logger.debug(f"Expected service names: {expected_service_names}")
        added_services = []
        service_details = {}

        for service_name in expected_service_names:
            service_info = next((s for s in services if getattr(s, "name", None) == service_name), None)
            logger.debug(f"Service {service_name}: {'found' if service_info else 'not found'}")
            if service_info:
                added_services.append(service_name)
                service_tools = [t for t in tools if getattr(t, "service_name", None) == service_name]
                service_details[service_name] = {
                    "tools_count": len(service_tools),
                    "status": getattr(service_info, "status", "unknown")
                }
                logger.debug(f"Service {service_name} has {len(service_tools)} tools")

        failed_services = [name for name in expected_service_names if name not in added_services]
        success = len(added_services) > 0
        total_tools = sum(details["tools_count"] for details in service_details.values())

        logger.debug(f"Successfully added services: {added_services}")
        logger.debug(f"Failed to add services: {failed_services}")

        message = (
            f"Successfully added {len(added_services)} service(s) with {total_tools} tools"
            if success else
            f"Failed to add services. Available services: {[getattr(s, 'name', 'unknown') for s in services]}"
        )

        return {
            "success": success,
            "added_services": added_services,
            "failed_services": failed_services,
            "service_details": service_details,
            "total_services": len(added_services),
            "total_tools": total_tools,
            "message": message
        }

    def _preprocess_service_config(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> Union[Dict[str, Any], List[Dict[str, Any]], str]:
        """预处理服务配置"""
        if not config:
            return config

        if isinstance(config, dict):
            # 处理单个服务配置
            # 兼容大小写不敏感的 mcpServers
            normalized = self._normalize_mcp_servers(config)
            if normalized:
                # mcpServers格式，返回标准化后的配置
                return normalized
            else:
                # 单个服务格式，进行验证和转换
                processed = config.copy()

                # 验证必需字段
                if "name" not in processed:
                    raise ValueError("服务配置缺少name字段")

                # 验证互斥字段
                if "url" in processed and "command" in processed:
                    raise ValueError("Cannot specify both url and command")

                # 自动推断transport类型
                if "url" in processed and "transport" not in processed:
                    url = processed["url"]
                    if "/sse" in url.lower():
                        processed["transport"] = "streamable_http"
                    else:
                        processed["transport"] = "streamable_http"

                # 验证args格式
                if "command" in processed and not isinstance(processed.get("args", []), list):
                    raise ValueError("Args must be a list")

                return processed

        return config

    def _extract_service_names(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> List[str]:
        """从配置中提取服务名称"""
        if not config:
            return []

        if isinstance(config, dict):
            if "name" in config:
                return [config["name"]]
            else:
                # 兼容大小写不敏感的 mcpServers
                key = self._find_mcp_servers_key(config)
                if key:
                    return list(config[key].keys())
        elif isinstance(config, list):
            return config

        return []

    async def add_service_async(self,
                               config: Union[ServiceConfigUnion, Dict[str, Any], List[Dict[str, Any]], str, None] = None,
                               json_file: str = None,
                               # 认证参数（可选；若上层已标准化可忽略）
                               auth: Optional[str] = None,
                               token: Optional[str] = None,
                               api_key: Optional[str] = None,
                               headers: Optional[Dict[str, str]] = None) -> 'MCPStoreContext':
        """
        增强版的服务添加方法，支持多种配置格式：
        1. URL方式：
           await add_service({
               "name": "weather",
               "url": "https://weather-api.example.com/mcp",
               "transport": "streamable_http"
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

        4. 不再支持“服务名称列表方式”，请传入完整配置（字典列表）或 mcpServers 字典。

        5. 不再支持“无参数方式”的全量注册（初始化阶段已同步一次）。

        6. JSON文件方式：
           await add_service(json_file="path/to/config.json")  # 读取JSON文件作为配置

        所有新添加的服务都会同步到 mcp.json 配置文件中。

        Args:
            config: 服务配置（字典/JSON字符串/包含 mcpServers 的字典/字典列表）
            json_file: JSON文件路径，如果指定则读取该文件作为配置
            auth/token/api_key/headers: 认证参数，会被标准化为 headers 并仅以 headers 落盘

        Returns:
            MCPStoreContext: 返回自身实例以支持链式调用
        """
        try:
            # 应用认证配置到服务配置中（token/api_key/auth -> headers）
            config = self._apply_auth_to_config(config, auth, token, api_key, headers)


            # 处理json_file参数（可选）
            if json_file is not None:
                logger.info(f"从JSON文件读取配置: {json_file}")
                try:
                    import json
                    import os

                    if not os.path.exists(json_file):
                        raise Exception(f"JSON文件不存在: {json_file}")

                    with open(json_file, 'r', encoding='utf-8') as f:
                        file_config = json.load(f)

                    logger.info(f"成功读取JSON文件，配置: {file_config}")

                    # 如果同时指定了config和json_file，优先使用json_file
                    if config is not None:
                        logger.warning("同时指定了config和json_file参数，将使用json_file")

                    config = file_config

                except Exception as e:
                    raise Exception(f"读取JSON文件失败: {e}")

            # 支持 config 传入 JSON 字符串（单服务或 mcpServers/root 映射）
            if isinstance(config, str):
                try:
                    import json as _json
                    cfg = _json.loads(config)
                    config = cfg
                except Exception:
                    raise Exception("config 为字符串时必须是合法的 JSON")

            # 宽容 root 映射（无 mcpServers）：{"svc": {"url"|"command"...}, ...}
            # 兼容大小写不敏感的 mcpServers
            if isinstance(config, dict) and not self._find_mcp_servers_key(config) and "name" not in config:
                if config and all(isinstance(v, dict) and ("url" in v or "command" in v) for v in config.values()):
                    config = {"mcpServers": config}

            # 必须提供配置
            if config is None and json_file is None:
                raise Exception("必须提供服务配置（字典/JSON字符串或 json_file）")

        except Exception as e:
            logger.error(f"参数处理失败: {e}")
            raise

        try:
            # 获取正确的 agent_id（Store级别使用global_agent_store作为agent_id）
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.orchestrator.client_manager.global_agent_store_id

            #  新增：详细的注册开始日志（已移除 source 参数）
            logger.info(f"[ADD_SERVICE] start")
            logger.info(f"[ADD_SERVICE] config type={type(config)} content={config}")
            logger.info(f"[ADD_SERVICE] context={self._context_type.name} agent_id={agent_id}")

            # 处理不同的输入格式
            if config is None:
                # 不再支持空参数的全量同步；初始化阶段已同步一次
                raise Exception("必须提供服务配置（不再支持空参数全量同步）")

            # 处理列表格式
            elif isinstance(config, list):
                if not config:
                    raise Exception("列表为空")

                # 判断是服务名称列表还是服务配置列表
                if all(isinstance(item, str) for item in config):
                    raise Exception("不支持以服务名称列表的方式添加，请传入完整配置（字典列表）或 mcpServers 字典")

                elif all(isinstance(item, dict) for item in config):
                    # 批量服务配置列表
                    logger.info(f"批量服务配置注册，数量: {len(config)}")

                    # 转换为MCPConfig格式
                    mcp_config = {"mcpServers": {}}
                    for service_config in config:
                        service_name = service_config.get("name")
                        if not service_name:
                            raise Exception("批量配置中的服务缺少name字段")
                        mcp_config["mcpServers"][service_name] = {
                            k: v for k, v in service_config.items() if k != "name"
                        }

                    # 将config设置为转换后的mcp_config，然后继续处理
                    config = mcp_config

                else:
                    raise Exception("列表中的元素类型不一致，必须全部是字符串（服务名称）或全部是字典（服务配置）")

            # 处理字典格式的配置（包括从批量配置转换来的）
            if isinstance(config, dict):
                # ========== 使用新架构 AsyncShell 执行 ==========
                # [架构说明] 2024-12 重构
                # 
                # 本方法现在使用新架构 ServiceManagementAsyncShell，与同步方法 add_service() 
                # 共享同一套 Core + Shell 实现，确保：
                # 
                # 1. 同步/异步路径统一：
                #    - add_service() [同步] → SyncShell → AOB 桥接 → AsyncShell → pykv
                #    - add_service_async() [异步] → AsyncShell → pykv（直接调用，无需桥接）
                # 
                # 2. 避免同步/异步混用问题：
                #    旧实现会触发 AOB 事件循环冲突，已完全替换为纯异步外壳。
                # 
                # 3. API 模式作为 SDK 薄封装：
                #    FastAPI 路由可以直接调用本方法，无需担心事件循环冲突。
                
                logger.info(f"[ADD_SERVICE_ASYNC] 使用新架构 AsyncShell 添加服务")

                # 确保异步外壳已初始化（与同步方法共享）
                if not hasattr(self, '_service_management_async_shell'):
                    from ..architecture import ServiceManagementFactory
                    sync_shell, async_shell, core = ServiceManagementFactory.create_service_management(
                        self._store.registry,
                        self._store.orchestrator,
                        agent_id=self._agent_id or self._store.client_manager.global_agent_store_id
                    )
                    self._service_management_sync_shell = sync_shell
                    self._service_management_async_shell = async_shell

                # 直接调用异步外壳（无需 AOB 桥接，已在事件循环中）
                result = await self._service_management_async_shell.add_service_async(config)

                logger.debug(f"[ADD_SERVICE_ASYNC] 结果: {result.get('success', False)}")
                return self

        except Exception as e:
            logger.error(f"服务添加失败: {e}")
            raise

    async def _initialize_service_tool_status(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        初始化服务的工具状态（使用 StateManager）
        
        Store 和 Agent 模式都需要调用此方法。
        所有工具默认状态为 "available"。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Raises:
            RuntimeError: 如果初始化失败
        """
        logger.debug(
            f"[TOOL_STATUS_INIT] 开始初始化工具状态: "
            f"agent_id={agent_id}, service_name={service_name}"
        )
        
        # 1. 获取服务的全局名称
        if self._context_type == ContextType.AGENT:
            # Agent 模式：需要将本地服务名映射到全局服务名
            service_global_name = self._store.registry.get_global_name_from_agent_service(
                agent_id, service_name
            )
            if not service_global_name:
                raise RuntimeError(
                    f"无法获取服务全局名称: agent_id={agent_id}, "
                    f"service_name={service_name}"
                )
        else:
            # Store 模式：服务名就是全局名称
            service_global_name = service_name
        
        logger.debug(
            f"[TOOL_STATUS_INIT] 服务全局名称: "
            f"service_name={service_name}, service_global_name={service_global_name}"
        )
        
        # 2. 从关系层获取服务的工具列表
        state_manager = self._store.registry._cache_state_manager
        relation_manager = self._store.registry._relation_manager
        
        tool_relations = await relation_manager.get_service_tools(service_global_name)
        
        if not tool_relations:
            logger.warning(
                f"[TOOL_STATUS_INIT] 服务没有工具: "
                f"service_global_name={service_global_name}"
            )
            # 即使没有工具，也要创建服务状态
            tool_relations = []
        
        # 3. 构建工具状态列表（所有工具默认 available）
        tools_status = []
        for tool_rel in tool_relations:
            tool_global_name = tool_rel.get("tool_global_name")
            tool_original_name = tool_rel.get("tool_original_name")
            
            if not tool_global_name or not tool_original_name:
                raise RuntimeError(
                    f"工具关系数据不完整: tool_rel={tool_rel}"
                )
            
            tools_status.append({
                "tool_global_name": tool_global_name,
                "tool_original_name": tool_original_name,
                "status": "available"
            })
        
        # 4. 使用 StateManager 更新服务状态
        await state_manager.update_service_status(
            service_global_name=service_global_name,
            health_status="initializing",
            tools_status=tools_status
        )
        
        logger.info(
            f"[TOOL_STATUS_INIT] 工具状态初始化成功: "
            f"service_global_name={service_global_name}, "
            f"tools_count={len(tools_status)}"
        )

    async def _connect_and_update_cache(self, agent_id: str, service_name: str, service_config: Dict[str, Any]):
        """异步连接服务并更新缓存状态"""
        try:
            # 🔗 新增：连接开始日志
            logger.debug(f"Connecting to service: {service_name}")
            logger.debug(f"Agent ID: {agent_id}")
            logger.info(f"🔗 [CONNECT_SERVICE] 调用orchestrator.connect_service")

            #  修复：使用connect_service方法（现已修复ConfigProcessor问题）
            try:
                logger.info(f"🔗 [CONNECT_SERVICE] 准备调用connect_service，参数: name={service_name}, agent_id={agent_id}")
                logger.info(f"🔗 [CONNECT_SERVICE] service_config: {service_config}")

                # 使用修复后的connect_service方法（现在会使用ConfigProcessor）
                success, message = await self._store.orchestrator.connect_service(
                    service_name, service_config=service_config, agent_id=agent_id
                )

                logger.debug("Service connection completed")

            except Exception as connect_error:
                logger.error(f"🔗 [CONNECT_SERVICE] connect_service调用异常: {connect_error}")
                import traceback
                logger.error(f"🔗 [CONNECT_SERVICE] 异常堆栈: {traceback.format_exc()}")
                success, message = False, f"Connection call failed: {connect_error}"

            # 🔗 新增：连接结果日志
            logger.info(f"🔗 [CONNECT_SERVICE] 连接结果: success={success}, message={message}")

            if success:
                logger.info(f"🔗 Service '{service_name}' connected successfully")
                # 连接成功，缓存会自动更新（通过现有的连接逻辑）
            else:
                logger.warning(f" Service '{service_name}' connection failed: {message}")
                # 将连接失败交给生命周期管理器处理（事件驱动）
                try:
                    from mcpstore.core.events.service_events import ServiceConnectionFailed

                    bus = getattr(self._store.orchestrator, "event_bus", None)
                    if bus:
                        failed_event = ServiceConnectionFailed(
                            agent_id=agent_id,
                            service_name=service_name,
                            error_message=message or "",
                            error_type="connection_failed",
                            retry_count=0,
                        )
                        await bus.publish(failed_event, wait=True)
                        logger.debug(f"🔗 [CONNECT_SERVICE] Published ServiceConnectionFailed for '{service_name}'")
                    else:
                        logger.warning("🔗 [CONNECT_SERVICE] EventBus not available; cannot publish ServiceConnectionFailed")
                except Exception as event_err:
                    logger.warning(f"🔗 [CONNECT_SERVICE] Failed to publish ServiceConnectionFailed: {event_err}")

        except Exception as e:
            logger.error(f"🔗 [CONNECT_SERVICE] 整个连接过程发生异常: {e}")
            import traceback
            logger.error(f"🔗 [CONNECT_SERVICE] 异常堆栈: {traceback.format_exc()}")

            # 通过事件驱动方式通知生命周期管理器异常结果
            try:
                from mcpstore.core.events.service_events import ServiceConnectionFailed

                bus = getattr(self._store.orchestrator, "event_bus", None)
                if bus:
                    failed_event = ServiceConnectionFailed(
                        agent_id=agent_id,
                        service_name=service_name,
                        error_message=str(e),
                        error_type="connection_exception",
                        retry_count=0,
                    )
                    await bus.publish(failed_event, wait=True)
                    logger.error(f"🔗 [CONNECT_SERVICE] Published ServiceConnectionFailed after exception for '{service_name}'")
                else:
                    logger.warning("🔗 [CONNECT_SERVICE] EventBus not available; cannot publish ServiceConnectionFailed after exception")
            except Exception as event_err:
                logger.warning(f"🔗 [CONNECT_SERVICE] Failed to publish ServiceConnectionFailed after exception: {event_err}")

    # ===  Service Initialization Methods ===

    def init_service(self, client_id_or_service_name: str = None, *,
                     client_id: str = None, service_name: str = None) -> 'MCPStoreContext':
        raise RuntimeError("[SERVICE_OPERATIONS] 同步 init_service 已禁用，请使用 init_service_async。")

    async def init_service_async(self, client_id_or_service_name: str = None, *,
                                client_id: str = None, service_name: str = None) -> 'MCPStoreContext':
        """异步版本的服务初始化"""
        try:
            # 1. 参数验证和标准化
            identifier = self._validate_and_normalize_init_params(
                client_id_or_service_name, client_id, service_name
            )

            # 2. 根据上下文类型确定 agent_id
            if self._context_type == ContextType.STORE:
                agent_id = self._store.client_manager.global_agent_store_id
            else:
                agent_id = self._agent_id

            # 3. 智能解析标识符（复用现有的完善逻辑）
            resolved_client_id, resolved_service_name = await self._resolve_client_id_or_service_name_async(
                identifier, agent_id
            )

            logger.info(f" [INIT_SERVICE] 解析结果: client_id={resolved_client_id}, service_name={resolved_service_name}")

            # 4. 从缓存获取服务配置
            service_config = await self._get_service_config_from_cache_async(agent_id, resolved_service_name)
            if not service_config:
                raise ValueError(f"Service configuration not found for {resolved_service_name}")

            # 5. 调用生命周期管理器初始化服务（通过 bridge 执行，保持单一事件循环）
            success = await self._store.for_store()._run_async_via_bridge(  # type: ignore[attr-defined]
                self._store.container.service_application_service.initialize_service(
                    agent_id=agent_id,
                    service_name=resolved_service_name,
                    service_config=service_config,
                    wait_timeout=0.0,
                ),
                op_name="service_operations.init_service_initialize"
            )

            if not success:
                raise RuntimeError(f"Failed to initialize service {resolved_service_name}")

            logger.info(f" [INIT_SERVICE] Service {resolved_service_name} initialized to INITIALIZING state")
            return self

        except Exception as e:
            logger.error(f" [INIT_SERVICE] Failed to initialize service: {e}")
            raise

    def _validate_and_normalize_init_params(self, client_id_or_service_name: str = None,
                                          client_id: str = None, service_name: str = None) -> str:
        """
        验证和标准化初始化参数

        Args:
            client_id_or_service_name: 通用标识符
            client_id: 明确的client_id
            service_name: 明确的service_name

        Returns:
            str: 标准化后的标识符

        Raises:
            ValueError: 参数验证失败时
        """
        # 统计非空参数数量
        params = [client_id_or_service_name, client_id, service_name]
        non_empty_params = [p for p in params if p is not None and p.strip()]

        if len(non_empty_params) == 0:
            raise ValueError("必须提供以下参数之一: client_id_or_service_name, client_id, service_name")

        if len(non_empty_params) > 1:
            raise ValueError("只能提供一个参数，不能同时使用多个参数")

        # 返回非空的参数
        if client_id_or_service_name:
            logger.debug(f" [INIT_PARAMS] 使用通用参数: {client_id_or_service_name}")
            return client_id_or_service_name.strip()
        elif client_id:
            logger.debug(f" [INIT_PARAMS] 使用明确client_id: {client_id}")
            return client_id.strip()
        elif service_name:
            logger.debug(f" [INIT_PARAMS] 使用明确service_name: {service_name}")
            return service_name.strip()

        # 理论上不会到达这里
        raise ValueError("参数验证异常")

    def _resolve_client_id_or_service_name(self, client_id_or_service_name: str, agent_id: str) -> Tuple[str, str]:
        """
        智能解析client_id或服务名（复用现有逻辑）

        直接复用 ServiceManagementMixin 中的 _resolve_client_id 方法
        确保解析逻辑的一致性

        Args:
            client_id_or_service_name: 用户输入的标识符
            agent_id: Agent ID（用于范围限制）

        Returns:
            Tuple[str, str]: (client_id, service_name)

        Raises:
            ValueError: 当参数无法解析或不存在时
        """
        # 直接调用 ServiceManagementMixin 中的方法
        return self._resolve_client_id(client_id_or_service_name, agent_id)

    async def _resolve_client_id_or_service_name_async(self, client_id_or_service_name: str, agent_id: str) -> Tuple[str, str]:
        """
        智能解析（异步版本），直接调用 ServiceManagementMixin 的异步实现。
        """
        return await self._resolve_client_id_async(client_id_or_service_name, agent_id)


    async def _get_service_config_from_cache_async(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """从缓存获取服务配置（异步版本）"""
        try:
            # 方法1: 从 service_metadata 获取（优先）- 从 pykv 异步读取
            metadata = await self._store.registry._service_state_service.get_service_metadata_async(agent_id, service_name)
            if metadata and metadata.service_config:
                logger.debug(f" [CONFIG] 从metadata获取配置: {service_name}")
                return metadata.service_config

            # 方法2: 从 client_config 获取（备用）
            client_id = await self._store.registry.get_service_client_id_async(agent_id, service_name)
            if client_id:
                client_config = self._store.registry.get_client_config_from_cache(client_id)
                if client_config and 'mcpServers' in client_config:
                    service_config = client_config['mcpServers'].get(service_name)
                    if service_config:
                        logger.debug(f" [CONFIG] 从client_config获取配置: {service_name}")
                        return service_config

            logger.warning(f"[CONFIG] 未找到服务配置: {service_name} (agent: {agent_id})")
            return None

        except Exception as e:
            logger.error(f" [CONFIG] 获取服务配置失败 {service_name}: {e}")
            return None

    # ===  新增：Agent 透明代理方法 ===

    async def _add_agent_services_with_mapping(self, services_to_add: Dict[str, Any], agent_id: str):
        """
        Agent 服务添加的透明代理实现

        实现逻辑：
        1. 为每个服务生成全局名称（带后缀）
        2. 添加到 global_agent_store 缓存（全局名称）
        3. 添加到 Agent 缓存（本地名称）
        4. 建立双向映射关系
        5. 生成共享 Client ID
        6. 同步到持久化文件
        """
        try:
            logger.debug(f"Starting agent transparent proxy service addition for agent: {agent_id}")

            from .agent_service_mapper import AgentServiceMapper
            from mcpstore.core.models.service import ServiceConnectionState

            mapper = AgentServiceMapper(agent_id)

            for local_name, service_config in services_to_add.items():
                logger.info(f" [AGENT_PROXY] 处理服务: {local_name}")

                # 1. 生成全局名称
                global_name = mapper.to_global_name(local_name)
                logger.debug(f" [AGENT_PROXY] 服务名映射: {local_name} → {global_name}")

                # 2. 检查是否已存在同名服务
                existing_client_id = self._store.registry.get_service_client_id(agent_id, local_name)
                existing_global_client_id = self._store.registry.get_service_client_id(
                    self._store.client_manager.global_agent_store_id, global_name
                )

                if existing_client_id and existing_global_client_id:
                    # 同名服务已存在，更新配置而不是重新创建
                    logger.info(f" [AGENT_PROXY] 发现同名服务，更新配置: {local_name}")
                    client_id = existing_client_id

                    # 使用 preserve_mappings=True 来保留现有映射关系
                    self._store.registry.add_service(
                        agent_id=self._store.client_manager.global_agent_store_id,
                        name=global_name,
                        session=None,
                        tools=[],
                        service_config=service_config,
                        state=ServiceConnectionState.INITIALIZING,
                        preserve_mappings=True
                    )

                    self._store.registry.add_service(
                        agent_id=agent_id,
                        name=local_name,
                        session=None,
                        tools=[],
                        service_config=service_config,
                        state=ServiceConnectionState.INITIALIZING,
                        preserve_mappings=True
                    )

                    logger.debug(f"Service config updated: {local_name} (Client ID: {client_id})")
                else:
                    # 新服务，正常创建
                    logger.info(f" [AGENT_PROXY] 创建新服务: {local_name}")

                    #  修复：统一使用 ClientIDGenerator 生成共享 Client ID
                    from mcpstore.core.utils.id_generator import ClientIDGenerator
                    client_id = ClientIDGenerator.generate_deterministic_id(
                        agent_id=agent_id,
                        service_name=local_name,
                        service_config=service_config,
                        global_agent_store_id=self._store.client_manager.global_agent_store_id
                    )
                    logger.debug(f" [AGENT_PROXY] 生成确定性共享 Client ID: {client_id}")

                    # 3. 添加到 global_agent_store 缓存（全局名称）
                    self._store.registry.add_service(
                        agent_id=self._store.client_manager.global_agent_store_id,
                        name=global_name,
                        session=None,
                        tools=[],
                        service_config=service_config,
                        state=ServiceConnectionState.INITIALIZING
                    )
                    logger.debug(f" [AGENT_PROXY] 添加到 global_agent_store: {global_name}")

                    # 4. 添加到 Agent 缓存（本地名称）
                    self._store.registry.add_service(
                        agent_id=agent_id,
                        name=local_name,
                        session=None,
                        tools=[],
                        service_config=service_config,
                        state=ServiceConnectionState.INITIALIZING
                    )
                    logger.debug(f" [AGENT_PROXY] 添加到 Agent 缓存: {agent_id}:{local_name}")

                    # 5. 建立双向映射关系（新服务）
                    self._store.registry.add_agent_service_mapping(agent_id, local_name, global_name)
                    logger.debug(f" [AGENT_PROXY] 建立映射关系: {agent_id}:{local_name} ↔ {global_name}")
                    # 验证映射是否建立成功
                    verify_mapping = self._store.registry.get_global_name_from_agent_service(agent_id, local_name)
                    logger.debug(f" [AGENT_PROXY] 验证映射: {agent_id}:{local_name} -> {verify_mapping} (期望: {global_name})")

                # 6. 设置共享 Client ID 映射（新服务和同名服务都需要）
                self._store.registry._agent_client_service.add_service_client_mapping(
                    self._store.client_manager.global_agent_store_id, global_name, client_id
                )
                self._store.registry._agent_client_service.add_service_client_mapping(agent_id, local_name, client_id)
                logger.debug(f" [AGENT_PROXY] 设置共享 Client ID 映射: {client_id}")

                # 7. 使用事件驱动架构添加服务（新服务和同名服务都需要）
                # 委托给应用服务，通过事件总线协调各个管理器
                try:
                    result = await self._store.container.service_application_service.add_service(
                        agent_id=self._store.client_manager.global_agent_store_id,
                        service_name=global_name,
                        service_config=service_config,
                        wait_timeout=0.0  # 不等待，立即返回
                    )
                    if result.success:
                        logger.debug(f" [AGENT_PROXY] 事件驱动架构初始化成功(仅全局): {global_name}")
                    else:
                        logger.warning(f" [AGENT_PROXY] 事件驱动架构初始化失败: {result.error_message}")
                except Exception as e:
                    logger.error(f" [AGENT_PROXY] 事件驱动架构初始化异常: {e}")

                # 8. 初始化工具集（Agent 模式必需）
                try:
                    # 注意：工具状态初始化在 registry.add_service 中完成
                    # 因为此时工具尚未注册到关系层，需要等待服务连接成功后才能获取工具列表
                    logger.debug(
                        f"[TOOL_INIT] 工具状态将在服务连接成功后初始化: "
                        f"agent_id={agent_id}, service_name={local_name}"
                    )
                except Exception as init_error:
                    logger.error(
                        f"Agent 服务添加失败: "
                        f"agent_id={agent_id}, service_name={local_name}, error={init_error}"
                    )
                    raise

                logger.info(f" [AGENT_PROXY] Agent 服务添加完成: {local_name} → {global_name}")

            # 9. 同步到持久化文件
            await self._sync_agent_services_to_files(agent_id, services_to_add)

            logger.info(f" [AGENT_PROXY] Agent 透明代理添加完成，共处理 {len(services_to_add)} 个服务")

        except Exception as e:
            logger.error(f" [AGENT_PROXY] Agent 透明代理添加失败: {e}")
            raise

    async def _sync_agent_services_to_files(self, agent_id: str, services_to_add: Dict[str, Any]):
        """同步 Agent 服务到持久化文件（优化：使用 UnifiedConfigManager）"""
        try:
            logger.info(f" [AGENT_SYNC] 开始同步 Agent 服务到文件: {agent_id}")

            # 构建带后缀的服务配置字典
            from .agent_service_mapper import AgentServiceMapper
            mapper = AgentServiceMapper(agent_id)
            
            global_services = {}
            for local_name, service_config in services_to_add.items():
                global_name = mapper.to_global_name(local_name)
                global_services[global_name] = service_config
                logger.debug(f" [AGENT_SYNC] 准备添加到 mcp.json: {global_name}")

            # 使用 UnifiedConfigManager 批量添加服务（一次性保存 + 自动刷新缓存）
            success = self._store._unified_config.batch_add_services(global_services)
            
            if success:
                logger.info(f" [AGENT_SYNC] mcp.json 更新成功：已添加 {len(global_services)} 个服务，缓存已同步")
            else:
                logger.error(f"❌ [AGENT_SYNC] mcp.json 更新失败")

            # 单源模式：不再写分片文件，仅维护 mcp.json
            logger.info(f"ℹ️ [AGENT_SYNC] 单源模式下已禁用分片文件写入（agent_clients/client_services）")

        except Exception as e:
            logger.error(f" [AGENT_SYNC] 同步 Agent 服务到文件失败: {e}")
            raise

    async def _get_agent_service_view(self) -> List[ServiceInfo]:
        """
        获取 Agent 的服务视图（本地名称）

        透明代理（方案A）：不读取 Agent 命名空间缓存，
        直接基于映射从 global_agent_store 的缓存派生服务列表。
        """
        try:
            from mcpstore.core.models.service import ServiceInfo
            from mcpstore.core.models.service import ServiceConnectionState

            agent_services: List[ServiceInfo] = []
            agent_id = self._agent_id
            global_agent_id = self._store.client_manager.global_agent_store_id

            # 1) 通过映射获取该 Agent 的全局服务名集合
            global_service_names = self._store.registry.get_agent_services(agent_id)
            if not global_service_names:
                logger.info(f" [AGENT_VIEW] Agent {agent_id} 服务视图: 0 个服务（无映射）")
                return agent_services

            # 2) 遍历每个全局服务，从全局命名空间读取完整信息，并以本地名展示
            for global_name in global_service_names:
                # 解析出 (agent_id, local_name)
                mapping = self._store.registry.get_agent_service_from_global_name(global_name)
                if not mapping:
                    continue
                mapped_agent, local_name = mapping
                if mapped_agent != agent_id:
                    continue

                complete_info = self._store.registry.get_complete_service_info(global_agent_id, global_name)
                if not complete_info:
                    logger.debug(f"[AGENT_VIEW] 全局缓存中未找到服务: {global_name}")
                    continue

                # 状态转换
                # 额外诊断：记录全局与Agent缓存的状态对比
                try:
                    global_state_dbg = self._store.registry._service_state_service.get_service_state(global_agent_id, global_name)
                    agent_state_dbg = self._store.registry._service_state_service.get_service_state(agent_id, local_name)
                    logger.debug(f"[AGENT_VIEW] state_compare local='{local_name}' global='{global_name}' global_state='{getattr(global_state_dbg,'value',global_state_dbg)}' agent_state='{getattr(agent_state_dbg,'value',agent_state_dbg)}'")
                except Exception:
                    pass

                state = complete_info.get("state", ServiceConnectionState.DISCONNECTED)
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except Exception:
                        state = ServiceConnectionState.DISCONNECTED

                cfg = complete_info.get("config", {})
                tool_count = complete_info.get("tool_count", 0)

                # 透明代理：client_id 使用全局命名空间的 client_id
                service_info = ServiceInfo(
                    name=local_name,
                    status=state,
                    transport_type=self._store._infer_transport_type(cfg) if hasattr(self._store, '_infer_transport_type') else None,
                    client_id=complete_info.get("client_id"),
                    config=cfg,
                    tool_count=tool_count,
                    keep_alive=cfg.get("keep_alive", False),
                )
                agent_services.append(service_info)
                logger.debug(f" [AGENT_VIEW] derive '{local_name}' <- '{global_name}' tools={tool_count}")

            logger.info(f" [AGENT_VIEW] Agent {agent_id} 服务视图: {len(agent_services)} 个服务（派生）")
            return agent_services

        except Exception as e:
            logger.error(f" [AGENT_VIEW] 获取 Agent 服务视图失败: {e}")
            return []

    def _apply_auth_to_config(self, config,
                               auth: Optional[str],
                               token: Optional[str],
                               api_key: Optional[str],
                               headers: Optional[Dict[str, str]]):
        """将认证配置应用到服务配置中（入口标准化）
        - 将 token/auth 统一映射为 Authorization: Bearer <token>
        - 将 api_key 统一映射为 X-API-Key: <api_key>
        - headers 显式传入拥有最高优先级（覆盖前两者的相同键）
        - 最终仅保留 headers 持久化，移除 token/api_key/auth 字段，避免混乱
        """
        # 如果没有任何认证参数，直接返回原配置
        if auth is None and token is None and api_key is None and (not headers):
            return config

        # 构造标准化后的 headers
        normalized_headers: Dict[str, str] = {}
        # 兼容历史：auth 等价于 token（优先使用 token 覆盖 auth）
        eff_token = token if token else auth
        if eff_token:
            normalized_headers.setdefault("Authorization", f"Bearer {eff_token}")
        if api_key:
            normalized_headers.setdefault("X-API-Key", api_key)
        # 显式 headers 最高优先级
        if headers:
            normalized_headers.update(headers)

        # 应用到配置（支持单服务字典或 mcpServers 结构）
        def _apply_to_service_cfg(svc_cfg: Dict[str, Any]) -> Dict[str, Any]:
            cfg = (svc_cfg or {}).copy()
            # 合并 headers
            existing = dict(cfg.get("headers", {}) or {})
            existing.update(normalized_headers)
            cfg["headers"] = existing
            # 清理入口字段，避免落盘混乱
            for k in ("token", "api_key", "auth"):
                if k in cfg:
                    try:
                        del cfg[k]
                    except Exception:
                        cfg.pop(k, None)
            return cfg

        # 兼容大小写不敏感的 mcpServers
        key = self._find_mcp_servers_key(config) if isinstance(config, dict) else None
        if key and isinstance(config[key], dict):
            final_config = {"mcpServers": {}}
            for name, svc_cfg in config[key].items():
                if isinstance(svc_cfg, dict):
                    final_config["mcpServers"][name] = _apply_to_service_cfg(svc_cfg)
                else:
                    final_config["mcpServers"][name] = svc_cfg
            return final_config
        else:
            # 单服务或其他可迭代形式
            if isinstance(config, dict):
                return _apply_to_service_cfg(config)
            elif config is None:
                return {"headers": normalized_headers}
            else:
                base = dict(config) if hasattr(config, "__iter__") and not isinstance(config, str) else {}
                return _apply_to_service_cfg(base)
