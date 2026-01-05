"""
Persistence Manager - 持久化管理模块

负责服务配置的JSON文件持久化相关功能，包括：
1. 从mcp.json加载服务配置
2. 标准MCP配置字段的提取
3. 配置数据的解析和验证
4. 服务实体和关系的创建
"""

import logging
from typing import Dict, Any, Optional

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

        self._logger.info(f"Initializing PersistenceManager, namespace: {namespace}")

    def _legacy(self, method: str) -> None:
        raise_legacy_error(
            f"core_registry.PersistenceManager.{method}",
            "Use core/cache managers and shells for persistence workflows.",
        )

    def initialize(self) -> None:
        """初始化持久化管理器"""
        self._logger.info("PersistenceManager initialization completed")

    def cleanup(self) -> None:
        """清理持久化管理器资源"""
        try:
            # 清理引用
            self._unified_config = None
            self._service_manager = None
            self._relation_manager = None

            self._logger.info("PersistenceManager cleanup completed")
        except Exception as e:
            self._logger.error(f"PersistenceManager cleanup error: {e}")
            raise

    def set_unified_config(self, unified_config: Any) -> None:
        """
        设置统一配置管理器

        Args:
            unified_config: 统一配置管理器实例
        """
        self._legacy("set_unified_config")

    def set_managers(self, service_manager=None, relation_manager=None) -> None:
        """
        设置依赖的管理器

        Args:
            service_manager: 服务管理器
            relation_manager: 关系管理器
        """
        self._legacy("set_managers")

    def load_services_from_json(self) -> Dict[str, Any]:
        """
        从 mcp.json 读取服务配置并恢复服务实体（同步版本）

        Returns:
            加载结果统计信息

        Raises:
            RuntimeError: 如果 unified_config 未设置
        """
        self._legacy("load_services_from_json")

    async def load_services_from_json_async(self) -> Dict[str, Any]:
        """
        从 mcp.json 读取服务配置并恢复服务实体（异步版本）

        Returns:
            加载结果统计信息

        Raises:
            RuntimeError: 如果 unified_config 未设置
        """
        self._legacy("load_services_from_json_async")

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
        self._legacy("extract_standard_mcp_config")

    def validate_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        验证服务配置的有效性

        Args:
            service_config: 服务配置

        Returns:
            验证结果，包含 is_valid 和 errors 字段
        """
        self._legacy("validate_service_config")

    def get_service_config_summary(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        获取服务配置的摘要信息

        Args:
            service_config: 服务配置

        Returns:
            配置摘要信息
        """
        self._legacy("get_service_config_summary")

    def export_service_configs(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        导出服务配置（用于备份或迁移）

        Args:
            agent_id: 可选的agent_id过滤，如果为None则导出所有

        Returns:
            导出的配置数据
        """
        self._legacy("export_service_configs")

    def import_service_configs(self, import_data: Dict[str, Any], overwrite: bool = False) -> Dict[str, Any]:
        """
        导入服务配置（用于恢复或迁移）

        Args:
            import_data: 导入的配置数据
            overwrite: 是否覆盖已存在的服务

        Returns:
            导入结果统计信息
        """
        self._legacy("import_service_configs")

    def get_stats(self) -> Dict[str, Any]:
        """
        获取持久化管理器的统计信息

        Returns:
            统计信息字典
        """
        self._legacy("get_stats")
