"""
MCPOrchestrator Standalone Config Module
独立配置模块 - 包含独立配置适配器
"""

import logging

logger = logging.getLogger(__name__)

class StandaloneConfigMixin:
    """独立配置混入类"""

    def _create_standalone_mcp_config(self, config_manager):
        """
        创建独立的MCP配置对象

        Args:
            config_manager: 独立配置管理器

        Returns:
            兼容的MCP配置对象
        """
        class StandaloneMCPConfigAdapter:
            """独立配置适配器 - 兼容MCPConfig接口"""

            def __init__(self, config_manager):
                self.config_manager = config_manager
                self.json_path = ":memory:"  # 表示内存配置

            def load_config(self):
                """加载配置"""
                return self.config_manager.get_mcp_config()

            def get_service_config(self, name):
                """获取服务配置"""
                return self.config_manager.get_service_config(name)

            def save_config(self, config):
                """保存配置（内存模式下不执行实际保存）"""
                logger.info("Standalone mode: config save skipped (memory-only)")
                return True

            def add_service(self, name, config):
                """添加服务"""
                self.config_manager.add_service_config(name, config)
                return True

            def remove_service(self, name):
                """移除服务"""
                # 在独立模式下，我们可以从运行时配置中移除
                services = self.config_manager.get_all_service_configs()
                if name in services:
                    del services[name]
                    logger.info(f"Removed service '{name}' from standalone config")
                    return True
                return False

        return StandaloneMCPConfigAdapter(config_manager)
