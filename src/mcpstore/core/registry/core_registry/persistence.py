"""
Persistence Manager - 持久化管理模块

负责服务配置的JSON文件持久化相关功能，包括：
1. 从mcp.json加载服务配置
2. 标准MCP配置字段的提取
3. 配置数据的解析和验证
4. 服务实体和关系的创建
"""

import logging
from typing import Dict, Any, Optional, List, Set

from .base import PersistenceManagerInterface
from .errors import raise_legacy_error

logger = logging.getLogger(__name__)


class PersistenceManager(PersistenceManagerInterface):
    """
    持久化管理器实现

    职责：
    - 从JSON配置文件加载服务配置
    - 提取标准MCP配置字段
    - 处理服务配置的解析和验证
    - 管理服务实体和关系的创建
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 统一配置管理器（将在后续注入）
        self._unified_config = None

        # 管理器引用（将在后续注入）
        self._service_manager = None
        self._relation_manager = None

        # 标准 MCP 配置字段
        self._standard_mcp_fields = {
            'command', 'args', 'env', 'url',
            'transport_type', 'working_dir', 'keep_alive',
            'package_name', 'timeout', 'retry_count'
        }

        self._logger.info(f"初始化PersistenceManager，命名空间: {namespace}")

    def initialize(self) -> None:
        """初始化持久化管理器"""
        self._logger.info("PersistenceManager 初始化完成")

    def cleanup(self) -> None:
        """清理持久化管理器资源"""
        try:
            # 清理引用
            self._unified_config = None
            self._service_manager = None
            self._relation_manager = None

            self._logger.info("PersistenceManager 清理完成")
        except Exception as e:
            self._logger.error(f"PersistenceManager 清理时出错: {e}")
            raise

    def set_unified_config(self, unified_config: Any) -> None:
        """
        设置统一配置管理器

        Args:
            unified_config: 统一配置管理器实例
        """
        self._unified_config = unified_config
        self._logger.info("已设置统一配置管理器")

    def set_managers(self, service_manager=None, relation_manager=None) -> None:
        """
        设置依赖的管理器

        Args:
            service_manager: 服务管理器
            relation_manager: 关系管理器
        """
        self._service_manager = service_manager
        self._relation_manager = relation_manager
        self._logger.info("已设置依赖的管理器")

    def load_services_from_json(self) -> Dict[str, Any]:
        """
        从 mcp.json 读取服务配置并恢复服务实体（同步版本）

        Returns:
            加载结果统计信息

        Raises:
            RuntimeError: 如果 unified_config 未设置
        """
        import asyncio
        try:
            loop = asyncio.get_event_loop()
        except RuntimeError:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

        return loop.run_until_complete(self.load_services_from_json_async())

    async def load_services_from_json_async(self) -> Dict[str, Any]:
        """
        从 mcp.json 读取服务配置并恢复服务实体（异步版本）

        Returns:
            加载结果统计信息

        Raises:
            RuntimeError: 如果 unified_config 未设置
        """
        if not self._unified_config:
            raise RuntimeError(
                "UnifiedConfigManager 未设置，无法从 JSON 加载配置。"
                "请先调用 set_unified_config() 方法。"
            )

        if not self._service_manager or not self._relation_manager:
            raise RuntimeError(
                "依赖的管理器未设置，无法加载配置。"
                "请先调用 set_managers() 方法。"
            )

        logger.info("[JSON_LOAD] 开始从 mcp.json 加载服务配置...")

        # 统计信息
        stats = {
            "total": 0,
            "loaded": 0,
            "failed": 0,
            "errors": []
        }

        try:
            # 读取 mcp.json 配置
            mcp_config = self._unified_config.get_mcp_config()
            mcp_servers = mcp_config.get("mcpServers", {})

            stats["total"] = len(mcp_servers)

            if not mcp_servers:
                logger.info("[JSON_LOAD] mcp.json 中没有服务配置")
                return stats

            logger.info(f"[JSON_LOAD] 发现 {len(mcp_servers)} 个服务配置")

            # 遍历所有服务
            for service_global_name, service_config in mcp_servers.items():
                try:
                    # 解析全局名称，提取 agent_id 和 original_name
                    original_name, agent_id = self._naming.parse_service_global_name(
                        service_global_name
                    )

                    logger.debug(
                        f"[JSON_LOAD] 解析服务: global_name={service_global_name}, "
                        f"original_name={original_name}, agent_id={agent_id}"
                    )

                    # 检查服务是否已存在
                    existing = await self._service_manager.get_service(service_global_name)
                    if existing:
                        logger.debug(
                            f"[JSON_LOAD] 服务已存在，跳过: {service_global_name}"
                        )
                        stats["loaded"] += 1
                        continue

                    # 创建服务实体
                    await self._service_manager.create_service(
                        agent_id=agent_id,
                        original_name=original_name,
                        config=service_config
                    )

                    # 创建 Agent-Service 关系
                    client_id = f"client_{agent_id}_{original_name}"
                    await self._relation_manager.add_agent_service(
                        agent_id=agent_id,
                        service_original_name=original_name,
                        service_global_name=service_global_name,
                        client_id=client_id
                    )

                    # 创建服务状态
                    from ..models.service import ServiceConnectionState
                    await self._cache_layer.set_entity_state(
                        entity_type="service",
                        global_name=service_global_name,
                        state=ServiceConnectionState.DISCONNECTED
                    )

                    stats["loaded"] += 1
                    logger.info(f"[JSON_LOAD] 加载服务: {service_global_name}")

                except Exception as e:
                    error_msg = f"加载服务 {service_global_name} 失败: {str(e)}"
                    logger.error(f"[JSON_LOAD] {error_msg}")
                    stats["errors"].append(error_msg)
                    stats["failed"] += 1

            logger.info(
                f"[JSON_LOAD] 加载完成: 总计={stats['total']}, "
                f"成功={stats['loaded']}, 失败={stats['failed']}"
            )

        except Exception as e:
            error_msg = f"从JSON加载配置时发生错误: {str(e)}"
            logger.error(f"[JSON_LOAD] {error_msg}")
            stats["errors"].append(error_msg)
            stats["failed"] = stats["total"]

        return stats

    def extract_standard_mcp_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        提取标准的 MCP 配置字段，排除 MCPStore 特定的元数据

        Args:
            service_config: 完整的服务配置

        Returns:
            只包含标准 MCP 字段的配置字典

        Note:
            标准 MCP 配置字段包括:
            - command: 命令
            - args: 参数列表
            - env: 环境变量
            - url: HTTP 服务 URL
            - transport_type: 传输类型（可选）

            排除的 MCPStore 特定字段:
            - added_time: 添加时间
            - source_agent: 来源 Agent
            - service_global_name: 全局名称
            - service_original_name: 原始名称
            - 其他内部元数据字段
        """
        # 提取标准字段
        mcp_config = {}
        for field in self._standard_mcp_fields:
            if field in service_config:
                mcp_config[field] = service_config[field]

        # 如果没有任何标准字段，返回原始配置（可能是简化配置）
        if not mcp_config:
            # 检查是否是简化的命令字符串配置
            if 'command' in service_config:
                mcp_config['command'] = service_config['command']
            else:
                # 返回原始配置，保持兼容性
                mcp_config = service_config.copy()

        logger.debug(f"提取MCP配置: {len(service_config)} -> {len(mcp_config)} 个字段")
        return mcp_config

    def validate_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        验证服务配置的有效性

        Args:
            service_config: 服务配置

        Returns:
            验证结果，包含 is_valid 和 errors 字段
        """
        result = {
            "is_valid": True,
            "errors": [],
            "warnings": []
        }

        try:
            # 检查是否包含必要的连接信息
            has_command = 'command' in service_config
            has_url = 'url' in service_config

            if not (has_command or has_url):
                result["is_valid"] = False
                result["errors"].append("服务配置必须包含 'command' 或 'url' 字段")

            # 验证命令配置
            if has_command:
                command = service_config.get('command')
                if not isinstance(command, str):
                    result["is_valid"] = False
                    result["errors"].append("'command' 字段必须是字符串")

                # 验证参数
                args = service_config.get('args', [])
                if not isinstance(args, list):
                    result["errors"].append("'args' 字段必须是列表")
                    service_config['args'] = []

            # 验证URL配置
            if has_url:
                url = service_config.get('url')
                if not isinstance(url, str):
                    result["is_valid"] = False
                    result["errors"].append("'url' 字段必须是字符串")

                # 简单的URL格式验证
                if not url.startswith(('http://', 'https://')):
                    result["warnings"].append("'url' 应该以 http:// 或 https:// 开头")

            # 验证环境变量
            if 'env' in service_config:
                env = service_config['env']
                if not isinstance(env, dict):
                    result["errors"].append("'env' 字段必须是字典")
                    service_config['env'] = {}

            # 检查传输类型
            if 'transport_type' in service_config:
                transport_type = service_config['transport_type']
                valid_transports = ['stdio', 'http', 'websocket']
                if transport_type not in valid_transports:
                    result["warnings"].append(
                        f"未知的传输类型 '{transport_type}'，建议使用: {valid_transports}"
                    )

        except Exception as e:
            result["is_valid"] = False
            result["errors"].append(f"验证过程中发生错误: {str(e)}")

        return result

    def get_service_config_summary(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        获取服务配置的摘要信息

        Args:
            service_config: 服务配置

        Returns:
            配置摘要信息
        """
        summary = {
            "connection_type": "unknown",
            "has_env": False,
            "has_args": False,
            "transport_type": "stdio"
        }

        # 确定连接类型
        if 'command' in service_config:
            summary["connection_type"] = "command"
        elif 'url' in service_config:
            summary["connection_type"] = "http"

        # 检查环境变量
        if 'env' in service_config and service_config['env']:
            summary["has_env"] = True

        # 检查参数
        if 'args' in service_config and service_config['args']:
            summary["has_args"] = True

        # 获取传输类型
        if 'transport_type' in service_config:
            summary["transport_type"] = service_config['transport_type']

        return summary

    def export_service_configs(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        导出服务配置（用于备份或迁移）

        Args:
            agent_id: 可选的agent_id过滤，如果为None则导出所有

        Returns:
            导出的配置数据
        """
        export_data = {
            "export_time": None,
            "namespace": self._namespace,
            "services": {},
            "stats": {
                "total_services": 0,
                "agents": set()
            }
        }

        # 从缓存层获取服务数据并构建完整的导出信息
        try:
            # 获取所有服务名称
            service_names = self._cache_layer.get_service_names()

            for service_name in service_names:
                try:
                    # 获取服务信息
                    service_info = self._cache_layer.get_service_info(service_name)
                    if not service_info:
                        continue

                    # 如果指定了agent_id，进行过滤
                    if agent_id:
                        service_agent_id = service_info.get("agent_id")
                        if service_agent_id != agent_id:
                            continue

                    # 获取服务配置和元数据
                    metadata = self._cache_layer.get_entity_metadata("service", service_name)

                    # 构建导出数据
                    exported_service = {
                        "name": service_info.get("name", service_name),
                        "agent_id": service_info.get("agent_id"),
                        "state": service_info.get("state", "unknown"),
                        "config": metadata.get("config", {}) if metadata else {},
                        "tools": service_info.get("tools", []),
                        "registered_at": metadata.get("registered_at", datetime.now().isoformat()) if metadata else datetime.now().isoformat()
                    }

                    export_data["services"][service_name] = exported_service
                    export_data["stats"]["total_services"] += 1

                    # 统计agents
                    service_agent_id = service_info.get("agent_id")
                    if service_agent_id:
                        export_data["stats"]["agents"].add(service_agent_id)

                except Exception as e:
                    self._logger.error(f"导出服务失败 {service_name}: {e}")
                    continue

            # 设置导出时间并转换agents集合
            export_data["export_time"] = datetime.now().isoformat()
            export_data["stats"]["agents"] = list(export_data["stats"]["agents"])

        except Exception as e:
            self._logger.error(f"导出服务配置时出错: {e}")

        self._logger.info(f"导出服务配置完成: agent_id={agent_id}, 总数={export_data['stats']['total_services']}")
        return export_data

    def import_service_configs(self, import_data: Dict[str, Any], overwrite: bool = False) -> Dict[str, Any]:
        """
        导入服务配置（用于恢复或迁移）

        Args:
            import_data: 导入的配置数据
            overwrite: 是否覆盖已存在的服务

        Returns:
            导入结果统计信息
        """
        stats = {
            "total": 0,
            "imported": 0,
            "skipped": 0,
            "failed": 0,
            "errors": []
        }

        # 验证导入数据
        if not isinstance(import_data, dict) or "services" not in import_data:
            stats["errors"].append("无效的导入数据格式")
            return stats

        services = import_data.get("services", {})
        stats["total"] = len(services)

        self._logger.info(f"开始导入服务配置: 总数={stats['total']}, overwrite={overwrite}")

        # 实现完整的服务导入逻辑
        try:
            # 注入service_manager用于服务注册
            from .service_manager import ServiceManager

            # 创建临时的service_manager实例用于导入
            # 这里使用现有的缓存层和命名服务
            service_manager = ServiceManager(self._cache_layer, self._naming, self._namespace)
            service_manager.initialize()

            for service_name, service_data in services.items():
                try:
                    agent_id = service_data.get("agent_id")
                    if not agent_id:
                        stats["errors"].append(f"服务 {service_name} 缺少agent_id")
                        stats["failed"] += 1
                        continue

                    service_config = service_data.get("config", {})
                    tools = service_data.get("tools", [])

                    # 检查服务是否已存在
                    if not overwrite:
                        existing_info = self._cache_layer.get_service_info(service_name)
                        if existing_info:
                            self._logger.debug(f"跳过已存在的服务: {service_name}")
                            stats["skipped"] += 1
                            continue

                    # 注册服务
                    success = service_manager.register_service(
                        agent_id=agent_id,
                        service_name=service_data.get("name", service_name),
                        service_config=service_config,
                        tools=tools,
                        overwrite=overwrite
                    )

                    if success:
                        # 设置服务状态
                        state = service_data.get("state", "initialized")
                        self._cache_layer.set_service_state(service_name, state)

                        # 如果有注册时间，保存到元数据
                        # 注意：元数据保存功能暂时跳过，因为 _cache_layer 是 RedisStore，没有 set_entity_metadata 方法
                        # 如果需要保存元数据，应该通过 CacheLayerManager.put_state() 方法
                        # TODO: 如果需要此功能，需要注入 CacheLayerManager 实例
                        if "registered_at" in service_data:
                            self._logger.debug(f"跳过元数据保存（需要 CacheLayerManager）: {service_name}")

                        stats["imported"] += 1
                        self._logger.debug(f"成功导入服务: {service_name}")
                    else:
                        stats["failed"] += 1
                        stats["errors"].append(f"服务注册失败: {service_name}")

                except Exception as e:
                    error_msg = f"导入服务失败 {service_name}: {e}"
                    self._logger.error(error_msg)
                    stats["errors"].append(error_msg)
                    stats["failed"] += 1

            # 清理临时service_manager
            service_manager.cleanup()

        except Exception as e:
            error_msg = f"导入过程中出现系统错误: {e}"
            self._logger.error(error_msg)
            stats["errors"].append(error_msg)

        return stats

    def get_stats(self) -> Dict[str, Any]:
        """
        获取持久化管理器的统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "has_unified_config": self._unified_config is not None,
            "has_service_manager": self._service_manager is not None,
            "has_relation_manager": self._relation_manager is not None,
            "standard_fields_count": len(self._standard_mcp_fields)
        }
