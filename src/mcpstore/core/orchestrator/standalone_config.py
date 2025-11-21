"""
MCPOrchestrator Standalone Config Module
Standalone configuration module - contains standalone configuration adapter
"""

import logging

logger = logging.getLogger(__name__)

class StandaloneConfigMixin:
    """Standalone configuration mixin class"""

    def _create_standalone_mcp_config(self, config_manager):
        """
        Create standalone MCP configuration object

        Args:
            config_manager: Standalone configuration manager

        Returns:
            Compatible MCP configuration object
        """
        class StandaloneMCPConfigAdapter:
            """Standalone configuration adapter - compatible with MCPConfig interface"""

            def __init__(self, config_manager):
                self.config_manager = config_manager
                self.json_path = ":memory:"  # Indicates memory configuration

            def load_config(self):
                """Load configuration"""
                return self.config_manager.get_mcp_config()

            def get_service_config(self, name):
                """Get service configuration"""
                return self.config_manager.get_service_config(name)

            def save_config(self, config):
                """Save configuration (no actual save in memory mode)"""
                logger.info("Standalone mode: config save skipped (memory-only)")
                return True

            def add_service(self, name, config):
                """Add service"""
                self.config_manager.add_service_config(name, config)
                return True

            def remove_service(self, name):
                """Remove service"""
                # In standalone mode, we can remove from runtime configuration
                services = self.config_manager.get_all_service_configs()
                if name in services:
                    del services[name]
                    logger.info(f"Removed service '{name}' from standalone config")
                    return True
                return False

        return StandaloneMCPConfigAdapter(config_manager)
