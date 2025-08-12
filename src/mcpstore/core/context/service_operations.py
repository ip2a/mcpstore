"""
MCPStore Service Operations Module
Implementation of service-related operations
"""

import asyncio
import logging
import time
from typing import Dict, List, Optional, Any, Union, Tuple

from mcpstore.core.models.service import ServiceInfo, ServiceConfigUnion, ServiceConnectionState, TransportType
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

    def parse_wait_parameter(self, wait_param: Union[str, int, float]) -> float:
        """
        解析等待参数

        Args:
            wait_param: 等待参数，支持:
                - "auto": 自动根据服务类型判断
                - 数字: 毫秒数
                - 字符串数字: 毫秒数

        Returns:
            float: 等待时间（秒）
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
    """Service operations mixin class"""



    # === Core service interface ===
    def list_services(self) -> List[ServiceInfo]:
        """
        List services (synchronous version) - 纯缓存查询，立即返回
        - store context: aggregate services from all client_ids under global_agent_store
        - agent context: aggregate services from all client_ids under agent_id

        🚀 优化：直接返回缓存状态，不等待任何连接
        服务状态管理由生命周期管理器负责，查询和管理完全分离
        """
        # 直接返回缓存中的服务列表，不等待任何连接
        return self._sync_helper.run_async(self.list_services_async(), force_background=True)

    async def list_services_async(self) -> List[ServiceInfo]:
        """
        List services (asynchronous version)
        - store context: aggregate services from all client_ids under global_agent_store
        - agent context: aggregate services from all client_ids under agent_id (show original names)
        """
        if self._context_type == ContextType.STORE:
            return await self._store.list_services()
        else:
            # Agent mode: get global service list, then convert to local names
            global_services = await self._store.list_services(self._agent_id, agent_mode=True)

            # Use mapper to convert to local names
            if self._service_mapper:
                local_services = self._service_mapper.convert_service_list_to_local(global_services)
                return local_services
            else:
                return global_services

    def add_service(self, config: Union[ServiceConfigUnion, List[str], None] = None, json_file: str = None, source: str = "manual", wait: Union[str, int, float] = "auto") -> 'MCPStoreContext':
        """
        Enhanced service addition method (synchronous version), supports multiple configuration formats

        Args:
            config: Service configuration, supports multiple formats
            json_file: JSON文件路径，如果指定则读取该文件作为配置
            source: 调用来源标识，用于日志追踪
            wait: 等待连接完成的时间
                - "auto": 自动根据服务类型判断（远程2s，本地4s）
                - 数字: 等待时间（毫秒）
        """
        # 🔧 修复：使用后台循环来支持后台任务
        return self._sync_helper.run_async(
            self.add_service_async(config, json_file, source, wait),
            timeout=120.0,
            force_background=True  # 强制使用后台循环，确保后台任务不被取消
        )

    def add_service_with_details(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> Dict[str, Any]:
        """
        添加服务并返回详细信息（同步版本）

        Args:
            config: 服务配置

        Returns:
            Dict: 包含添加结果的详细信息
        """
        # 🔧 修复：使用后台循环来支持后台任务
        return self._sync_helper.run_async(
            self.add_service_with_details_async(config),
            timeout=120.0,
            force_background=True  # 强制使用后台循环
        )

    async def add_service_with_details_async(self, config: Union[Dict[str, Any], List[Dict[str, Any]], str] = None) -> Dict[str, Any]:
        """
        添加服务并返回详细信息（异步版本）

        Args:
            config: 服务配置

        Returns:
            Dict: 包含添加结果的详细信息
        """
        logger.info(f"[add_service_with_details_async] 开始添加服务，配置: {config}")

        # 预处理配置
        try:
            processed_config = self._preprocess_service_config(config)
            logger.info(f"[add_service_with_details_async] 预处理后的配置: {processed_config}")
        except ValueError as e:
            logger.error(f"[add_service_with_details_async] 预处理配置失败: {e}")
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
            logger.info(f"[add_service_with_details_async] 调用 add_service_async")
            result = await self.add_service_async(processed_config)
            logger.info(f"[add_service_with_details_async] add_service_async 结果: {result}")
        except Exception as e:
            logger.error(f"[add_service_with_details_async] add_service_async 失败: {e}")
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
            logger.error(f"[add_service_with_details_async] add_service_async 返回 None")
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
        logger.info(f"[add_service_with_details_async] 获取添加后的服务和工具列表")
        services = await self.list_services_async()
        tools = await self.list_tools_async()
        logger.info(f"[add_service_with_details_async] 当前服务数量: {len(services)}, 工具数量: {len(tools)}")
        logger.info(f"[add_service_with_details_async] 当前服务列表: {[getattr(s, 'name', 'unknown') for s in services]}")

        # 分析添加结果
        expected_service_names = self._extract_service_names(config)
        logger.info(f"[add_service_with_details_async] 期望的服务名称: {expected_service_names}")
        added_services = []
        service_details = {}

        for service_name in expected_service_names:
            service_info = next((s for s in services if getattr(s, "name", None) == service_name), None)
            logger.info(f"[add_service_with_details_async] 检查服务 {service_name}: {'找到' if service_info else '未找到'}")
            if service_info:
                added_services.append(service_name)
                service_tools = [t for t in tools if getattr(t, "service_name", None) == service_name]
                service_details[service_name] = {
                    "tools_count": len(service_tools),
                    "status": getattr(service_info, "status", "unknown")
                }
                logger.info(f"[add_service_with_details_async] 服务 {service_name} 有 {len(service_tools)} 个工具")

        failed_services = [name for name in expected_service_names if name not in added_services]
        success = len(added_services) > 0
        total_tools = sum(details["tools_count"] for details in service_details.values())

        logger.info(f"[add_service_with_details_async] 添加成功的服务: {added_services}")
        logger.info(f"[add_service_with_details_async] 添加失败的服务: {failed_services}")

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
            if "mcpServers" in config:
                # mcpServers格式，直接返回
                return config
            else:
                # 单个服务格式，进行验证和转换
                processed = config.copy()

                # 验证必需字段
                if "name" not in processed:
                    raise ValueError("Service name is required")

                # 验证互斥字段
                if "url" in processed and "command" in processed:
                    raise ValueError("Cannot specify both url and command")

                # 自动推断transport类型
                if "url" in processed and "transport" not in processed:
                    url = processed["url"]
                    if "/sse" in url.lower():
                        processed["transport"] = "sse"
                    else:
                        processed["transport"] = "streamable-http"

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
            elif "mcpServers" in config:
                return list(config["mcpServers"].keys())
        elif isinstance(config, list):
            return config

        return []

    async def add_service_async(self, config: Union[ServiceConfigUnion, List[str], None] = None, json_file: str = None, source: str = "manual", wait: Union[str, int, float] = "auto") -> 'MCPStoreContext':
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

            # 如果既没有config也没有json_file，且不是Store模式的全量注册，则报错
            if config is None and json_file is None and self._context_type != ContextType.STORE:
                raise Exception("必须指定config参数或json_file参数")

        except Exception as e:
            logger.error(f"参数处理失败: {e}")
            raise

        try:
            # 获取正确的 agent_id（Store级别使用global_agent_store作为agent_id）
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.orchestrator.client_manager.global_agent_store_id

            # 🔄 新增：详细的注册开始日志
            logger.info(f"🔄 [ADD_SERVICE] 开始注册服务 - 调用来源: {source}")
            logger.info(f"🔄 [ADD_SERVICE] 配置类型: {type(config)}, 配置内容: {config}")
            logger.info(f"🔄 [ADD_SERVICE] 上下文: {self._context_type.name}, Agent ID: {agent_id}")

            # 处理不同的输入格式
            if config is None:
                # Store模式下的全量注册
                if self._context_type == ContextType.STORE:
                    logger.info("STORE模式-使用统一同步机制注册所有服务")
                    # 🔧 修改：使用统一同步机制，不再手动注册
                    if hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                        results = await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
                        logger.info(f"同步结果: {results}")
                        if not (results.get("added") or results.get("updated")):
                            logger.warning("没有服务被同步，可能mcp.json为空或所有服务已是最新")
                    else:
                        logger.warning("统一同步管理器不可用，跳过同步")
                    return self
                else:
                    logger.warning("AGENT模式-未指定服务配置")
                    raise Exception("AGENT模式必须指定服务配置")

            # 处理列表格式
            elif isinstance(config, list):
                if not config:
                    raise Exception("列表为空")

                # 判断是服务名称列表还是服务配置列表
                if all(isinstance(item, str) for item in config):
                    # 服务名称列表
                    logger.info(f"注册指定服务: {config}")
                    if self._context_type == ContextType.STORE:
                        resp = await self._store.register_selected_services_for_store(config)
                    else:
                        resp = await self._store.register_services_for_agent(agent_id, config)
                    logger.info(f"注册结果: {resp}")
                    if not (resp and resp.service_names):
                        raise Exception("服务注册失败")
                    # 服务名称列表注册完成，直接返回
                    return self

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
                # 🔧 新增：缓存优先的添加服务流程
                return await self._add_service_cache_first(config, agent_id, wait)

        except Exception as e:
            logger.error(f"服务添加失败: {e}")
            raise

    async def _add_service_cache_first(self, config: Dict[str, Any], agent_id: str, wait: Union[str, int, float] = "auto") -> 'MCPStoreContext':
        """
        缓存优先的添加服务流程

        🔧 新流程：
        1. 立即更新缓存（用户马上可以查询）
        2. 尝试连接服务（更新缓存状态）
        3. 异步持久化到文件（不阻塞用户）
        """
        try:
            # 🔄 新增：缓存优先流程开始日志
            logger.info(f"🔄 [ADD_SERVICE] 进入缓存优先流程")

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

            # === 第1阶段：立即缓存操作（快速响应） ===
            logger.info(f"🔄 [ADD_SERVICE] 第1阶段: 立即缓存操作开始")
            services_to_add = mcp_config["mcpServers"]
            cache_results = []
            logger.info(f"🔄 [ADD_SERVICE] 待添加服务数量: {len(services_to_add)}")

            # 🔧 Agent模式下为服务名添加后缀
            if self._context_type == ContextType.AGENT:
                suffixed_services = {}
                for original_name, service_config in services_to_add.items():
                    suffixed_name = f"{original_name}by{self._agent_id}"
                    suffixed_services[suffixed_name] = service_config
                    logger.info(f"Agent服务名转换: {original_name} -> {suffixed_name}")
                services_to_add = suffixed_services

            for service_name, service_config in services_to_add.items():
                # 1.1 立即添加到缓存（初始化状态）
                cache_result = await self._add_service_to_cache_immediately(
                    agent_id, service_name, service_config
                )
                cache_results.append(cache_result)

                logger.info(f"✅ Service '{service_name}' added to cache immediately")

            # === 第2阶段：异步连接服务（更新缓存状态） ===
            logger.info(f"🔄 [ADD_SERVICE] 第2阶段: 异步连接任务创建开始")
            connection_tasks = []
            for service_name, service_config in services_to_add.items():
                logger.info(f"🔄 [ADD_SERVICE] 创建连接任务: {service_name}")
                task = asyncio.create_task(
                    self._connect_and_update_cache(agent_id, service_name, service_config)
                )
                connection_tasks.append(task)

            logger.info(f"🔄 [ADD_SERVICE] 已创建 {len(connection_tasks)} 个连接任务")

            # 🔧 修复：确保异步任务不被垃圾回收
            if not hasattr(self._store, '_background_tasks'):
                self._store._background_tasks = set()

            for task in connection_tasks:
                self._store._background_tasks.add(task)
                # 任务完成后自动从集合中移除
                task.add_done_callback(lambda t: self._store._background_tasks.discard(t))
                logger.info(f"🔄 [ADD_SERVICE] 任务已添加到后台任务集合: {task}")

            # === 第3阶段：异步持久化（不阻塞） ===
            logger.info(f"🔄 [ADD_SERVICE] 第3阶段: 异步持久化任务创建开始")
            # 使用锁防止并发持久化冲突
            if not hasattr(self, '_persistence_lock'):
                self._persistence_lock = asyncio.Lock()

            persistence_task = asyncio.create_task(
                self._persist_to_files_with_lock(mcp_config, services_to_add)
            )
            # 存储任务引用，避免被垃圾回收
            if not hasattr(self, '_persistence_tasks'):
                self._persistence_tasks = set()
            self._persistence_tasks.add(persistence_task)
            persistence_task.add_done_callback(self._persistence_tasks.discard)

            # === 第4阶段：可选的连接等待 ===
            if wait != "auto" or wait == "auto":  # 总是处理等待逻辑
                wait_timeout = self.wait_strategy.parse_wait_parameter(wait)

                if wait_timeout is None:  # auto模式
                    wait_timeout = self.wait_strategy.get_max_wait_timeout(services_to_add)

                if wait_timeout > 0:
                    logger.info(f"🔄 [ADD_SERVICE] 第4阶段: 等待连接完成，超时时间: {wait_timeout}s")

                    # 并发等待所有服务连接完成
                    service_names = list(services_to_add.keys())
                    final_states = await self._wait_for_services_ready(
                        agent_id, service_names, wait_timeout
                    )

                    logger.info(f"🔄 [ADD_SERVICE] 等待完成，最终状态: {final_states}")
                else:
                    logger.info(f"🔄 [ADD_SERVICE] 跳过等待，立即返回")

            logger.info(f"Added {len(services_to_add)} services to cache immediately, connecting in background")
            return self

        except Exception as e:
            logger.error(f"Cache-first add service failed: {e}")
            raise

    async def _wait_for_services_ready(self, agent_id: str, service_names: List[str], timeout: float) -> Dict[str, str]:
        """
        并发等待多个服务就绪

        Args:
            agent_id: Agent ID
            service_names: 服务名称列表
            timeout: 等待超时时间（秒）

        Returns:
            Dict[str, str]: 服务名称 -> 最终状态
        """

        async def wait_single_service(service_name: str) -> tuple[str, str]:
            """等待单个服务就绪"""
            start_time = time.time()
            logger.debug(f"🔄 [WAIT_SERVICE] 开始等待服务: {service_name}")

            while time.time() - start_time < timeout:
                try:
                    current_state = self._store.registry.get_service_state(agent_id, service_name)

                    # 如果状态已确定（不再是INITIALIZING），返回结果
                    if current_state and current_state != ServiceConnectionState.INITIALIZING:
                        elapsed = time.time() - start_time
                        logger.debug(f"✅ [WAIT_SERVICE] 服务{service_name}状态确定: {current_state.value} (耗时: {elapsed:.2f}s)")
                        return service_name, current_state.value

                    # 短暂等待后重试
                    await asyncio.sleep(0.1)

                except Exception as e:
                    logger.debug(f"⚠️ [WAIT_SERVICE] 检查服务{service_name}状态时出错: {e}")
                    await asyncio.sleep(0.1)

            # 超时，返回当前状态或超时状态
            try:
                current_state = self._store.registry.get_service_state(agent_id, service_name)
                final_state = current_state.value if current_state else 'timeout'
            except Exception:
                final_state = 'timeout'

            logger.warning(f"⏰ [WAIT_SERVICE] 服务{service_name}等待超时: {final_state}")
            return service_name, final_state

        # 并发等待所有服务
        logger.info(f"🔄 [WAIT_SERVICES] 开始并发等待{len(service_names)}个服务，超时: {timeout}s")
        tasks = [wait_single_service(name) for name in service_names]

        try:
            results = await asyncio.gather(*tasks, return_exceptions=True)

            # 处理结果
            final_states = {}
            for result in results:
                if isinstance(result, tuple) and len(result) == 2:
                    service_name, state = result
                    final_states[service_name] = state
                elif isinstance(result, Exception):
                    logger.error(f"❌ [WAIT_SERVICES] 等待服务时出现异常: {result}")
                    # 为异常的服务设置错误状态
                    for name in service_names:
                        if name not in final_states:
                            final_states[name] = 'error'
                            break

            logger.info(f"🔄 [WAIT_SERVICES] 并发等待完成: {final_states}")
            return final_states

        except Exception as e:
            logger.error(f"❌ [WAIT_SERVICES] 并发等待过程中出现异常: {e}")
            # 返回所有服务的错误状态
            return {name: 'error' for name in service_names}

    async def _add_service_to_cache_immediately(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """立即添加服务到缓存"""
        try:
            # 1. 生成或获取 client_id
            client_id = self._get_or_create_client_id(agent_id, service_name)

            # 2. 立即添加到所有相关缓存
            # 2.1 添加到服务缓存（初始化状态）
            from mcpstore.core.models.service import ServiceConnectionState
            self._store.registry.add_service(
                agent_id=agent_id,
                name=service_name,
                session=None,  # 暂无连接
                tools=[],      # 暂无工具
                service_config=service_config,
                state=ServiceConnectionState.INITIALIZING
            )

            # 2.2 添加到 Agent-Client 映射缓存
            self._store.registry.add_agent_client_mapping(agent_id, client_id)

            # 2.3 添加到 Client 配置缓存
            self._store.registry.add_client_config(client_id, {
                "mcpServers": {service_name: service_config}
            })

            # 2.4 添加到 Service-Client 映射缓存
            self._store.registry.add_service_client_mapping(agent_id, service_name, client_id)

            # 2.5 初始化到生命周期管理器
            self._store.orchestrator.lifecycle_manager.initialize_service(
                agent_id, service_name, service_config
            )

            return {
                "service_name": service_name,
                "client_id": client_id,
                "agent_id": agent_id,
                "status": "cached_immediately",
                "state": "initializing"
            }

        except Exception as e:
            logger.error(f"Failed to add {service_name} to cache immediately: {e}")
            raise

    def _get_or_create_client_id(self, agent_id: str, service_name: str) -> str:
        """生成或获取 client_id"""
        # 检查是否已有client_id
        existing_client_id = self._store.registry.get_service_client_id(agent_id, service_name)
        if existing_client_id:
            return existing_client_id

        # 生成新的client_id
        return self._store.client_manager.generate_client_id()

    async def _connect_and_update_cache(self, agent_id: str, service_name: str, service_config: Dict[str, Any]):
        """异步连接服务并更新缓存状态"""
        try:
            # 🔗 新增：连接开始日志
            logger.info(f"🔗 [CONNECT_SERVICE] 开始连接服务: {service_name}")
            logger.info(f"🔗 [CONNECT_SERVICE] Agent ID: {agent_id}")
            logger.info(f"🔗 [CONNECT_SERVICE] 调用orchestrator.connect_service")

            # 🔧 修复：使用connect_service方法（现已修复ConfigProcessor问题）
            try:
                logger.info(f"🔗 [CONNECT_SERVICE] 准备调用connect_service，参数: name={service_name}, agent_id={agent_id}")
                logger.info(f"🔗 [CONNECT_SERVICE] service_config: {service_config}")

                # 使用修复后的connect_service方法（现在会使用ConfigProcessor）
                success, message = await self._store.orchestrator.connect_service(
                    service_name, service_config=service_config, agent_id=agent_id
                )

                logger.info(f"🔗 [CONNECT_SERVICE] connect_service调用完成")

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
                logger.warning(f"❌ Service '{service_name}' connection failed: {message}")
                # 更新缓存状态为失败（不重复添加服务，只更新状态）
                from mcpstore.core.models.service import ServiceConnectionState
                self._store.registry.set_service_state(agent_id, service_name, ServiceConnectionState.DISCONNECTED)

                # 更新错误信息
                metadata = self._store.registry.get_service_metadata(agent_id, service_name)
                if metadata:
                    metadata.error_message = message
                    metadata.consecutive_failures += 1

        except Exception as e:
            logger.error(f"🔗 [CONNECT_SERVICE] 整个连接过程发生异常: {e}")
            import traceback
            logger.error(f"🔗 [CONNECT_SERVICE] 异常堆栈: {traceback.format_exc()}")

            # 更新缓存状态为错误（不重复添加服务，只更新状态）
            from mcpstore.core.models.service import ServiceConnectionState
            self._store.registry.set_service_state(agent_id, service_name, ServiceConnectionState.UNREACHABLE)

            # 更新错误信息
            metadata = self._store.registry.get_service_metadata(agent_id, service_name)
            if metadata:
                metadata.error_message = str(e)
                metadata.consecutive_failures += 1

            logger.error(f"🔗 [CONNECT_SERVICE] 服务状态已更新为UNREACHABLE: {service_name}")

    async def _persist_to_files_with_lock(self, mcp_config: Dict[str, Any], services_to_add: Dict[str, Dict[str, Any]]):
        """带锁的异步持久化到文件（防止并发冲突）"""
        async with self._persistence_lock:
            await self._persist_to_files_async(mcp_config, services_to_add)

    async def _persist_to_files_async(self, mcp_config: Dict[str, Any], services_to_add: Dict[str, Dict[str, Any]]):
        """异步持久化到文件（不阻塞用户）"""
        try:
            logger.info("📁 Starting background file persistence...")

            if self._context_type == ContextType.STORE:
                # Store模式：更新 mcp.json 和 agent_clients 映射
                await self._persist_to_mcp_json(services_to_add)
                # 🔧 修复：Store模式也需要同步agent_clients映射到文件
                await self._persist_store_agent_mappings(services_to_add)
            else:
                # Agent模式：更新 agent_clients.json 和 client_services.json
                await self._persist_to_agent_files(services_to_add)

            logger.info("📁 Background file persistence completed")

        except Exception as e:
            logger.error(f"Background file persistence failed: {e}")
            # 文件持久化失败不影响缓存使用，但需要记录

    async def _persist_to_mcp_json(self, services_to_add: Dict[str, Dict[str, Any]]):
        """持久化到 mcp.json"""
        try:
            # 1. 加载现有配置
            current_config = self._store.config.load_config()

            # 2. 合并新配置到mcp.json
            for name, service_config in services_to_add.items():
                current_config["mcpServers"][name] = service_config

            # 3. 保存更新后的配置
            self._store.config.save_config(current_config)

            # 4. 重新加载配置以确保同步
            self._store.config.load_config()

            logger.info("Store模式：mcp.json已更新")

        except Exception as e:
            logger.error(f"Failed to persist to mcp.json: {e}")
            raise

    async def _persist_store_agent_mappings(self, services_to_add: Dict[str, Dict[str, Any]]):
        """
        Store模式：持久化agent_clients映射到文件

        Store模式下，服务添加到global_agent_store，需要同步映射关系到文件
        """
        try:
            agent_id = self._store.client_manager.global_agent_store_id
            logger.info(f"🔄 Store模式agent映射持久化开始，agent_id: {agent_id}, 服务数量: {len(services_to_add)}")

            # 触发缓存到文件的同步
            logger.info("🔄 触发agent_clients缓存到文件同步")
            cache_manager = getattr(self._store, 'cache_manager', None)
            if cache_manager:
                cache_manager.sync_to_client_manager(self._store.client_manager)
                logger.info("✅ 使用cache_manager同步完成")
            else:
                # 备用方案：直接调用registry的同步方法
                logger.info("🔄 使用备用方案：registry直接同步")
                self._store.registry.sync_to_client_manager(self._store.client_manager)
                logger.info("✅ 使用registry直接同步完成")

            logger.info("✅ Store模式agent映射持久化完成")

        except Exception as e:
            logger.error(f"Failed to persist store agent mappings: {e}")
            # 不抛出异常，因为这不应该阻止服务添加

    async def _persist_to_agent_files(self, services_to_add: Dict[str, Dict[str, Any]]):
        """
        持久化到 Agent 文件（新逻辑：增量操作缓存，然后缓存同步到文件）

        新流程：
        1. 增量更新缓存中的映射关系（使用services_to_add参数）
        2. 触发缓存到文件的同步
        """
        try:
            agent_id = self._agent_id
            logger.info(f"🔄 Agent模式持久化开始，agent_id: {agent_id}, 服务数量: {len(services_to_add)}")

            # 1. 增量更新缓存映射（而不是全量同步）
            for service_name, service_config in services_to_add.items():
                # 获取或创建client_id
                client_id = self._get_or_create_client_id(agent_id, service_name)

                # 更新Agent-Client映射缓存
                if agent_id not in self._store.registry.agent_clients:
                    self._store.registry.agent_clients[agent_id] = []
                if client_id not in self._store.registry.agent_clients[agent_id]:
                    self._store.registry.agent_clients[agent_id].append(client_id)

                # 更新Client配置缓存
                self._store.registry.client_configs[client_id] = {
                    "mcpServers": {service_name: service_config}
                }

                logger.info(f"✅ 缓存更新完成: {service_name} -> {client_id}")

            # 2. 触发缓存到文件的同步
            logger.info("🔄 触发缓存到文件同步")
            cache_manager = getattr(self._store, 'cache_manager', None)
            if cache_manager:
                cache_manager.sync_to_client_manager(self._store.client_manager)
            else:
                # 备用方案
                self._store.registry.sync_to_client_manager(self._store.client_manager)

            logger.info("✅ Agent模式：缓存增量更新并同步到文件完成")

        except Exception as e:
            logger.error(f"Failed to persist to agent files with incremental cache update: {e}")
            raise

    # === 🆕 Service Initialization Methods ===

    def init_service(self, client_id_or_service_name: str = None, *,
                     client_id: str = None, service_name: str = None) -> 'MCPStoreContext':
        """
        初始化服务到 INITIALIZING 状态

        支持三种调用方式（只能使用其中一种）：
        1. 通用参数：init_service("identifier")
        2. 明确client_id：init_service(client_id="client_123")
        3. 明确service_name：init_service(service_name="weather")

        Args:
            client_id_or_service_name: 通用标识符（客户端ID或服务名称）
            client_id: 明确指定的客户端ID（关键字参数）
            service_name: 明确指定的服务名称（关键字参数）

        Returns:
            MCPStoreContext: 支持链式调用

        Usage:
            # Store级别
            store.for_store().init_service("weather")                    # 通用方式
            store.for_store().init_service(client_id="client_123")       # 明确client_id
            store.for_store().init_service(service_name="weather")       # 明确service_name

            # Agent级别（自动处理名称映射）
            store.for_agent("agent1").init_service("weather")           # 通用方式
            store.for_agent("agent1").init_service(client_id="client_456") # 明确client_id
            store.for_agent("agent1").init_service(service_name="weather") # 明确service_name
        """
        return self._sync_helper.run_async(
            self.init_service_async(client_id_or_service_name, client_id=client_id, service_name=service_name),
            timeout=30.0,
            force_background=True
        )

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
            resolved_client_id, resolved_service_name = self._resolve_client_id_or_service_name(
                identifier, agent_id
            )

            logger.info(f"🔍 [INIT_SERVICE] 解析结果: client_id={resolved_client_id}, service_name={resolved_service_name}")

            # 4. 从缓存获取服务配置
            service_config = self._get_service_config_from_cache(agent_id, resolved_service_name)
            if not service_config:
                raise ValueError(f"Service configuration not found for {resolved_service_name}")

            # 5. 调用生命周期管理器初始化服务
            success = self._store.orchestrator.lifecycle_manager.initialize_service(
                agent_id, resolved_service_name, service_config
            )

            if not success:
                raise RuntimeError(f"Failed to initialize service {resolved_service_name}")

            logger.info(f"✅ [INIT_SERVICE] Service {resolved_service_name} initialized to INITIALIZING state")
            return self

        except Exception as e:
            logger.error(f"❌ [INIT_SERVICE] Failed to initialize service: {e}")
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
            logger.debug(f"🔍 [INIT_PARAMS] 使用通用参数: {client_id_or_service_name}")
            return client_id_or_service_name.strip()
        elif client_id:
            logger.debug(f"🔍 [INIT_PARAMS] 使用明确client_id: {client_id}")
            return client_id.strip()
        elif service_name:
            logger.debug(f"🔍 [INIT_PARAMS] 使用明确service_name: {service_name}")
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


    def _get_service_config_from_cache(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """从缓存获取服务配置"""
        try:
            # 方法1: 从 service_metadata 获取（优先）
            metadata = self._store.registry.get_service_metadata(agent_id, service_name)
            if metadata and metadata.service_config:
                logger.debug(f"🔍 [CONFIG] 从metadata获取配置: {service_name}")
                return metadata.service_config

            # 方法2: 从 client_config 获取（备用）
            client_id = self._store.registry.get_service_client_id(agent_id, service_name)
            if client_id:
                client_config = self._store.registry.get_client_config_from_cache(client_id)
                if client_config and 'mcpServers' in client_config:
                    service_config = client_config['mcpServers'].get(service_name)
                    if service_config:
                        logger.debug(f"🔍 [CONFIG] 从client_config获取配置: {service_name}")
                        return service_config

            logger.warning(f"⚠️ [CONFIG] 未找到服务配置: {service_name} (agent: {agent_id})")
            return None

        except Exception as e:
            logger.error(f"❌ [CONFIG] 获取服务配置失败 {service_name}: {e}")
            return None
