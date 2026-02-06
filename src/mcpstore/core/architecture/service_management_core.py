"""
Service Management Core - 纯同步业务逻辑核心

根据Functional Core, Imperative Shell架构原则：
- 纯同步执行
- 不包含任何IO操作
- 不调用任何异步方法
- 只返回操作计划，不执行实际操作
"""

import logging
from dataclasses import dataclass
from typing import Dict, Any, List

logger = logging.getLogger(__name__)


@dataclass
class ServiceOperation:
    """单个服务操作的数据结构"""
    type: str  # "put_entity", "put_relation", "update_state"
    collection: str  # pykv collection名称
    key: str  # pykv key
    data: Dict[str, Any]  # 操作数据


@dataclass
class ServiceOperationPlan:
    """服务操作计划"""
    operations: List[ServiceOperation]
    service_names: List[str]  # 涉及的服务名称列表

    def __post_init__(self):
        """后处理：提取服务名称"""
        if not self.service_names:
            self.service_names = [
                op.data.get("service_original_name") or op.data.get("service_name")
                for op in self.operations
                if op.data.get("service_original_name") or op.data.get("service_name")
            ]


@dataclass
class WaitOperationPlan:
    """等待服务操作计划"""
    service_name: str
    global_name: str
    target_status: str
    timeout: float
    check_interval: float


class ServiceManagementCore:
    """
    纯同步的服务管理核心逻辑

    严格遵循Functional Core原则：
    - Pure synchronous execution
    - No IO operations
    - No async method calls
    - No await/asyncio.run()
    - Only return operation plans, do not execute actual operations
    """

    def __init__(self, agent_id: str = "global_agent_store"):
        """初始化核心逻辑"""
        logger.debug("[SERVICE_CORE] [INIT] Initializing ServiceManagementCore")
        self.agent_id = agent_id

    def add_service(self, config: Dict[str, Any]) -> ServiceOperationPlan:
        """
        纯同步：解析服务配置，生成操作计划

        Args:
            config: 服务配置，支持多种格式：
                - {"mcpServers": {"service1": {...}, "service2": {...}}}
                - {"name": "service1", "url": "...", ...}
                - 字符串URL

        Returns:
            ServiceOperationPlan: 包含所有需要执行的操作计划
        """
        logger.debug(f"[SERVICE_CORE] [PARSE] Starting to parse service configuration: {type(config).__name__}")

        # 1. 解析配置，标准化为服务字典
        service_configs = self._parse_service_config(config)

        if not service_configs:
            raise ValueError("Invalid service configuration, unable to parse any services")

        # 2. 生成操作计划
        operations = []
        service_names = []

        for service_name, service_config in service_configs.items():
            logger.debug(f"[SERVICE_CORE] [PROCESS] Processing service: {service_name}")

            # 使用当前上下文的 agent_id（store=global_agent_store，agent上下文则为具体ID）
            agent_id = self.agent_id or "global_agent_store"
            # 使用NamingService生成全局名称，确保与缓存层一致
            from ..cache.naming_service import NamingService
            naming = NamingService()
            global_name = naming.generate_service_global_name(service_name, agent_id)

            # 构建服务实体数据
            service_entity_data = self._build_service_entity_data(
                agent_id=agent_id,
                original_name=service_name,
                global_name=global_name,
                config=service_config
            )

            # 操作1: 创建服务实体
            operations.append(ServiceOperation(
                type="put_entity",
                collection="default:entity:services",
                key=global_name,
                data={
                    "key": global_name,  # 缓存层需要的 key
                    "value": service_entity_data,  # 缓存层需要的 value
                    # 保留原有数据以备其他用途
                    "agent_id": agent_id,
                    "original_name": service_name,
                    "global_name": global_name,
                    "config": service_config
                }
            ))

            # 操作2: 创建Agent-Service关系
            client_id = f"client_{agent_id}_{service_name}"
            relation_data = {
                "agent_id": agent_id,
                "service_name": service_name,
                "client_id": client_id,
                "global_name": global_name
            }
            operations.append(ServiceOperation(
                type="put_relation",
                collection="default:relations:agent_services",
                key=f"{agent_id}:{service_name}",
                data={
                    "key": f"{agent_id}:{service_name}",  # 缓存层需要的 key
                    "value": relation_data,  # 缓存层需要的 value
                    # 保留原有数据以备其他用途
                    "agent_id": agent_id,
                    "service_original_name": service_name,
                    "service_global_name": global_name,
                    "client_id": client_id,
                    "relation_data": relation_data
                }
            ))

            # 操作3: 初始化服务状态
            import time
            state_data = {
                "service_global_name": global_name,  # 必需字段
                "health_status": "startup",  # 初始状态：正在初始化
                "last_health_check": int(time.time()),  # 必需字段
                "connection_attempts": 0,  # 必需字段
                "max_connection_attempts": 3,  # 必需字段
                "current_error": None,  # 可选字段
                "tools": []  # 工具状态列表
            }
            operations.append(ServiceOperation(
                type="update_state",
                collection="default:state:service_status",
                key=global_name,
                data={
                    "key": global_name,  # 缓存层需要的 key
                    "value": state_data,  # 缓存层需要的 value
                    # 保留原有数据以备其他用途
                    "global_name": global_name,
                    "health_status": "startup",
                    "tools_status": [],  # 兼容性保留
                    "error_message": None,
                    "last_heartbeat": None
                }
            ))

            # 操作4: 初始化 service_metadata
            metadata_data = {
                "service_global_name": global_name,
                "agent_id": agent_id,
                "created_time": int(time.time()),
                "state_entered_time": int(time.time()),
                "reconnect_attempts": 0,
                "last_ping_time": None,
            }
            operations.append(ServiceOperation(
                type="put_metadata",
                collection="default:state:service_metadata",
                key=global_name,
                data={
                    "key": global_name,
                    "value": metadata_data,
                }
            ))

            service_names.append(service_name)

        logger.debug(f"[SERVICE_CORE] [PLAN] Generated operation plan: {len(operations)} operations, {len(service_names)} services")

        return ServiceOperationPlan(
            operations=operations,
            service_names=service_names
        )

    def wait_service_plan(self, service_name: str, timeout: float = 40.0) -> WaitOperationPlan:
        """
        纯同步：生成等待服务的操作计划

        Args:
            service_name: 服务名称
            timeout: 超时时间

        Returns:
            WaitOperationPlan: 等待操作计划
        """
        logger.debug(f"[SERVICE_CORE] [PLAN] Generated wait plan: {service_name}, timeout={timeout}")

        agent_id = self.agent_id or "global_agent_store"
        # 使用NamingService生成全局名称，确保与缓存层一致
        from ..cache.naming_service import NamingService
        naming = NamingService()
        global_name = naming.generate_service_global_name(service_name, agent_id)

        return WaitOperationPlan(
            service_name=service_name,
            global_name=global_name,
            target_status="healthy",
            timeout=timeout,
            check_interval=0.5  # 每0.5秒检查一次
        )

    # ===================== 私有辅助方法 =====================

    def _parse_service_config(self, config: Dict[str, Any]) -> Dict[str, Dict[str, Any]]:
        """
        纯同步：解析各种格式的服务配置

        支持格式：
        1. {"mcpServers": {"service1": {...}, "service2": {...}}}
        2. {"name": "service1", "url": "...", ...}
        3. {"service_name": {"url": "...", ...}, ...}
        """
        if not isinstance(config, dict):
            raise ValueError(f"Configuration must be a dictionary type, actual type: {type(config).__name__}")

        # 格式1: mcpServers格式
        if "mcpServers" in config:
            mcp_servers = config["mcpServers"]
            if not isinstance(mcp_servers, dict):
                raise ValueError("mcpServers must be a dictionary type")
            return mcp_servers

        # 格式2: 单个服务配置（有name字段）
        if "name" in config:
            service_name = config["name"]
            if not isinstance(service_name, str):
                raise ValueError("Service name must be a string")
            return {service_name: config}

        # 格式3: 直接是服务字典
        # 假设所有值都是服务配置
        service_configs = {}
        for key, value in config.items():
            if isinstance(value, dict) and ("url" in value or "command" in value):
                service_configs[key] = value

        if service_configs:
            return service_configs

        raise ValueError("Unrecognized service configuration format")

    def _generate_global_name(self, service_name: str, agent_id: str) -> str:
        """纯同步：生成全局服务名称"""
        return f"{agent_id}::{service_name}"

    def _build_service_entity_data(self, agent_id: str, original_name: str, global_name: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """纯同步：构建服务实体数据"""
        import time

        return {
            "service_global_name": global_name,
            "service_original_name": original_name,
            "source_agent": agent_id,
            "config": config,
            "added_time": int(time.time()),
            "transport_type": self._infer_transport_type(config),
            "tool_count": 0  # 初始为0，后续可能更新
        }

    def _infer_transport_type(self, config: Dict[str, Any]) -> str:
        """纯同步：推断传输类型"""
        # 优先检查transport字段
        transport = config.get("transport")
        if transport:
            return str(transport)

        # 检查URL
        if config.get("url"):
            return "streamable_http"

        # 检查命令
        cmd = (config.get("command") or "").lower()
        args = " ".join(config.get("args", [])).lower()

        if "npx" in cmd or "node" in cmd or "npm" in cmd:
            return "stdio"
        if "python" in cmd or "pip" in cmd or ".py" in args:
            return "stdio"

        return "streamable_http"  # 默认
