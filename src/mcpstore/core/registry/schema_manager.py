"""
Schema Manager - Unified management of MCPStore configuration templates and validation rules

Provides configuration template loading, data validation, template retrieval and other functions, replacing hardcoded configuration templates.
"""

import json
import logging
from pathlib import Path
from typing import Dict, Any, Optional, List
from datetime import datetime

logger = logging.getLogger(__name__)


class SchemaManager:
    """Schema Manager - Unified management of configuration templates and validation rules"""

    def __init__(self):
        """Initialize Schema Manager"""
        self.schemas_dir = Path(__file__).parent.parent / "data" / "schemas"
        self._schemas_cache: Dict[str, Dict[str, Any]] = {}
        self._load_all_schemas()
    
    def _load_all_schemas(self) -> None:
        """Load all Schema files to cache"""
        try:
            schema_files = [
                "mcp_config.json",
                "agent_clients.json", 
                "client_services.json",
                "service_templates.json"
            ]
            
            for schema_file in schema_files:
                schema_path = self.schemas_dir / schema_file
                if schema_path.exists():
                    with open(schema_path, 'r', encoding='utf-8') as f:
                        schema_data = json.load(f)
                        schema_name = schema_file.replace('.json', '')
                        self._schemas_cache[schema_name] = schema_data
                        logger.debug(f"Loaded schema: {schema_name}")
                else:
                    logger.warning(f"Schema file not found: {schema_path}")
            
            logger.info(f"Loaded {len(self._schemas_cache)} schema files")
            
        except Exception as e:
            logger.error(f"Failed to load schemas: {e}")
            # If loading fails, use default templates
            self._load_fallback_schemas()
    
    def _load_fallback_schemas(self) -> None:
        """加载默认的fallback模板（兼容性保证）"""
        logger.warning("Using fallback schemas due to loading failure")
        
        self._schemas_cache = {
            "mcp_config": {
                "template": {
                    "mcpServers": {},
                    "version": "1.0.0",
                    "created_by": "MCPStore",
                    "created_at": None,
                    "description": "MCPStore configuration file"
                }
            },
            "agent_clients": {
                "template": {}
            },
            "client_services": {
                "template": {}
            },
            "service_templates": {
                "remote_http": {
                    "template": {
                        "name": "",
                        "url": "",
                        "transport": "streamable-http",
                        "headers": {},
                        "timeout": 30
                    }
                },
                "local_python": {
                    "template": {
                        "name": "",
                        "command": "python",
                        "args": [],
                        "env": {},
                        "working_dir": ""
                    }
                }
            }
        }
    
    def get_template(self, schema_name: str, template_name: Optional[str] = None) -> Dict[str, Any]:
        """
        获取配置模板

        Args:
            schema_name: Schema名称 (如: mcp_config, agent_clients)
            template_name: 模板名称 (如: remote_http, local_python)

        Returns:
            Dict[str, Any]: 模板数据
        """
        try:
            schema = self._schemas_cache.get(schema_name, {})

            if template_name:
                # 对于service_templates，需要从properties中获取
                if schema_name == "service_templates":
                    properties = schema.get("properties", {})
                    template_data = properties.get(template_name, {})
                    return template_data.get("template", {})
                else:
                    # 其他schema直接获取
                    template_data = schema.get(template_name, {})
                    return template_data.get("template", {})
            else:
                # 获取默认模板
                return schema.get("template", {})

        except Exception as e:
            logger.error(f"Failed to get template {schema_name}.{template_name}: {e}")
            return {}
    
    def get_mcp_config_template(self) -> Dict[str, Any]:
        """获取MCP配置文件模板"""
        template = self.get_template("mcp_config")
        if template and template.get("created_at") is None:
            template = template.copy()
            template["created_at"] = datetime.now().isoformat()
        return template
    
    def get_agent_clients_template(self) -> Dict[str, Any]:
        """获取Agent客户端映射模板"""
        return self.get_template("agent_clients")
    
    def get_client_services_template(self) -> Dict[str, Any]:
        """获取客户端服务配置模板"""
        return self.get_template("client_services")
    
    def get_service_template(self, service_type: str) -> Dict[str, Any]:
        """
        获取服务配置模板
        
        Args:
            service_type: 服务类型 (remote_http, local_python, local_node, local_npx)
            
        Returns:
            Dict[str, Any]: 服务模板
        """
        return self.get_template("service_templates", service_type)
    
    def get_known_service_config(self, service_name: str) -> Dict[str, Any]:
        """
        获取已知服务的配置

        Args:
            service_name: 服务名称 (如: mcpstore-wiki, howtocook)

        Returns:
            Dict[str, Any]: 服务配置
        """
        try:
            service_templates = self._schemas_cache.get("service_templates", {})
            properties = service_templates.get("properties", {})
            known_services = properties.get("known_services", {})
            known_properties = known_services.get("properties", {})
            service_config = known_properties.get(service_name, {})
            return service_config.get("template", {})
        except Exception as e:
            logger.error(f"Failed to get known service config {service_name}: {e}")
            return {}
    
    def list_service_templates(self) -> List[str]:
        """获取所有可用的服务模板类型"""
        try:
            service_templates = self._schemas_cache.get("service_templates", {})
            properties = service_templates.get("properties", {})
            templates = []
            for key in properties.keys():
                if key not in ["known_services"]:
                    templates.append(key)
            return templates
        except Exception as e:
            logger.error(f"Failed to list service templates: {e}")
            return ["remote_http", "local_python", "local_node", "local_npx"]
    
    def validate_config(self, schema_name: str, config_data: Dict[str, Any]) -> bool:
        """
        验证配置数据是否符合Schema
        
        Args:
            schema_name: Schema名称
            config_data: 要验证的配置数据
            
        Returns:
            bool: 验证是否通过
        """
        try:
            # 简单的基础验证，可以后续扩展为完整的JSON Schema验证
            schema = self._schemas_cache.get(schema_name, {})
            
            if schema_name == "mcp_config":
                return isinstance(config_data, dict) and "mcpServers" in config_data
            elif schema_name == "agent_clients":
                return isinstance(config_data, dict)
            elif schema_name == "client_services":
                return isinstance(config_data, dict)
            else:
                return isinstance(config_data, dict)
                
        except Exception as e:
            logger.error(f"Config validation failed for {schema_name}: {e}")
            return False
    
    def reload_schemas(self) -> bool:
        """重新加载所有Schema文件"""
        try:
            self._schemas_cache.clear()
            self._load_all_schemas()
            logger.info("Successfully reloaded all schemas")
            return True
        except Exception as e:
            logger.error(f"Failed to reload schemas: {e}")
            return False


# 全局Schema管理器实例
_schema_manager: Optional[SchemaManager] = None


def get_schema_manager() -> SchemaManager:
    """获取全局Schema管理器实例（单例模式）"""
    global _schema_manager
    if _schema_manager is None:
        _schema_manager = SchemaManager()
    return _schema_manager
