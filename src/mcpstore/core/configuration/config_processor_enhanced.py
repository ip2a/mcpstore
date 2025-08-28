#!/usr/bin/env python3
"""
Enhanced Configuration Processor - User-friendly configuration processing
Lenient to users, strict to FastMCP, supports multiple input formats
"""

import logging
from copy import deepcopy
from typing import Dict, Any, Union, List

from ..registry.schema_manager import get_schema_manager

logger = logging.getLogger(__name__)

class ConfigProcessor:
    """
    Enhanced configuration processor: handles conversion between user configuration and FastMCP configuration

    Design philosophy:
    1. Extremely user-friendly: supports multiple input formats
    2. Strict to FastMCP: ensures format fully complies with requirements
    3. Intelligent inference: automatically handles transport, environment variables, etc.
    4. Error-friendly: provides clear error messages and suggestions
    """
    
    # Standard fields supported by FastMCP
    FASTMCP_REMOTE_FIELDS = {
        "url", "transport", "headers", "timeout", "keep_alive", "auth"
    }
    
    FASTMCP_LOCAL_FIELDS = {
        "command", "args", "env", "cwd", "timeout", "keep_alive"
    }
    
    # Supported transport types
    VALID_TRANSPORTS = {
        "http", "streamable-http", "sse", "stdio"
    }
    
    # Use Schema manager to get known service configurations
    @classmethod
    def _get_known_services(cls) -> Dict[str, Dict[str, Any]]:
        """Get known service configurations"""
        schema_manager = get_schema_manager()
        return {
            "mcpstore-wiki": schema_manager.get_known_service_config("mcpstore-wiki"),
            "howtocook": schema_manager.get_known_service_config("howtocook")
        }

    @classmethod
    def normalize_user_input(cls, user_input: Union[str, List[str], Dict[str, Any]]) -> Dict[str, Any]:
        """
        将各种用户输入格式标准化为MCP配置格式
        
        支持的输入格式：
        1. 字符串: "mcpstore-wiki"
        2. 字符串列表: ["mcpstore-wiki", "howtocook"]
        3. 部分配置: {"mcpstore-wiki": {"url": "..."}}
        4. 完整配置: {"mcpServers": {...}}
        5. 混合配置: {"mcpServers": {...}, "services": [...]}
        
        Args:
            user_input: 用户输入
            
        Returns:
            标准化的MCP配置
        """
        try:
            # 1. 字符串输入
            if isinstance(user_input, str):
                return cls._handle_string_input(user_input)
            
            # 2. 字符串列表输入
            if isinstance(user_input, list):
                return cls._handle_list_input(user_input)
            
            # 3. 字典输入
            if isinstance(user_input, dict):
                return cls._handle_dict_input(user_input)
            
            # 4. 其他类型
            logger.warning(f"Unsupported input type: {type(user_input)}, treating as empty config")
            return {"mcpServers": {}}
            
        except Exception as e:
            logger.error(f"Failed to normalize user input: {e}")
            return {"mcpServers": {}}

    @classmethod
    def _handle_string_input(cls, service_name: str) -> Dict[str, Any]:
        """处理字符串输入"""
        config = cls._get_service_config(service_name)
        return {"mcpServers": {service_name: config}}

    @classmethod
    def _handle_list_input(cls, service_names: List[str]) -> Dict[str, Any]:
        """处理字符串列表输入"""
        mcp_servers = {}
        for service_name in service_names:
            if isinstance(service_name, str):
                config = cls._get_service_config(service_name)
                mcp_servers[service_name] = config
            else:
                logger.warning(f"Skipping non-string service name: {service_name}")
        
        return {"mcpServers": mcp_servers}

    @classmethod
    def _handle_dict_input(cls, user_dict: Dict[str, Any]) -> Dict[str, Any]:
        """处理字典输入"""
        # 如果已经是标准MCP格式
        if "mcpServers" in user_dict:
            result = deepcopy(user_dict)
            
            # 处理额外的服务列表字段
            if "services" in user_dict:
                additional_services = cls._handle_list_input(user_dict["services"])
                result["mcpServers"].update(additional_services["mcpServers"])
                del result["services"]
            
            return result
        
        # 否则假设是服务配置字典
        mcp_servers = {}
        for service_name, service_config in user_dict.items():
            if isinstance(service_config, dict):
                mcp_servers[service_name] = service_config
            elif isinstance(service_config, str):
                # 处理简化的URL配置
                mcp_servers[service_name] = {"url": service_config}
            else:
                logger.warning(f"Skipping invalid service config for '{service_name}': {service_config}")
        
        return {"mcpServers": mcp_servers}

    @classmethod
    def _get_service_config(cls, service_name: str) -> Dict[str, Any]:
        """获取服务配置，优先使用已知服务的默认配置"""
        known_services = cls._get_known_services()
        if service_name in known_services:
            logger.debug(f"Using known configuration for service: {service_name}")
            return deepcopy(known_services[service_name])
        
        # 尝试智能推断
        if service_name.startswith("http://") or service_name.startswith("https://"):
            # 用户直接提供了URL
            return {"url": service_name}
        
        # 默认假设是需要查找的服务
        logger.warning(f"Unknown service '{service_name}', you may need to provide configuration")
        return {"url": f"http://unknown-service/{service_name}"}

    @classmethod
    def process_for_fastmcp(cls, user_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        将用户配置转换为FastMCP兼容的配置
        
        Args:
            user_config: 用户配置
            
        Returns:
            FastMCP兼容的配置
        """
        if not isinstance(user_config, dict) or "mcpServers" not in user_config:
            logger.warning("Invalid config format, attempting to normalize")
            user_config = cls.normalize_user_input(user_config)
        
        # 深拷贝避免修改原配置
        fastmcp_config = deepcopy(user_config)
        
        # 处理每个服务
        services_to_remove = []
        for service_name, service_config in fastmcp_config["mcpServers"].items():
            try:
                processed_config = cls._process_single_service(service_config)
                fastmcp_config["mcpServers"][service_name] = processed_config
                logger.debug(f"Successfully processed service '{service_name}' for FastMCP")
            except Exception as e:
                logger.error(f"Failed to process service '{service_name}': {e}")
                services_to_remove.append(service_name)
        
        # 移除有问题的服务
        for service_name in services_to_remove:
            del fastmcp_config["mcpServers"][service_name]
        
        return fastmcp_config

    @classmethod
    def _process_single_service(cls, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """处理单个服务配置"""
        if not isinstance(service_config, dict):
            raise ValueError("Service config must be a dictionary")
        
        processed = deepcopy(service_config)
        
        # 判断服务类型并处理
        if "url" in processed:
            return cls._process_remote_service(processed)
        elif "command" in processed:
            return cls._process_local_service(processed)
        else:
            raise ValueError("Service config missing both 'url' and 'command'")

    @classmethod
    def _process_remote_service(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """处理远程服务配置"""
        # 智能推断transport
        config = cls._infer_transport(config)
        
        # 处理认证信息
        config = cls._process_auth(config)
        
        # 只保留FastMCP支持的字段
        fastmcp_config = {
            key: value for key, value in config.items() 
            if key in cls.FASTMCP_REMOTE_FIELDS
        }
        
        # 确保必要字段存在
        if "url" not in fastmcp_config:
            raise ValueError("Remote service missing required 'url' field")
        
        return fastmcp_config

    @classmethod
    def _process_local_service(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """处理本地服务配置"""
        # 移除transport字段（本地服务不需要）
        if "transport" in config:
            config = deepcopy(config)
            del config["transport"]
        
        # 处理环境变量
        config = cls._process_env_vars(config)
        
        # 只保留FastMCP支持的字段
        fastmcp_config = {
            key: value for key, value in config.items() 
            if key in cls.FASTMCP_LOCAL_FIELDS
        }
        
        # 确保必要字段存在
        if "command" not in fastmcp_config:
            raise ValueError("Local service missing required 'command' field")
        
        return fastmcp_config

    @classmethod
    def _infer_transport(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """智能推断transport字段"""
        config = deepcopy(config)
        url = config.get("url", "")
        
        # 如果用户已经指定了transport，验证并保留
        if "transport" in config:
            transport = config["transport"]
            if transport in cls.VALID_TRANSPORTS:
                return config
            else:
                logger.warning(f"Invalid transport '{transport}', will auto-infer")
                del config["transport"]
        
        # 自动推断transport
        if "/sse" in url.lower() or url.endswith("/sse"):
            config["transport"] = "sse"
        elif "streamable" in url.lower():
            config["transport"] = "streamable-http"
        else:
            # 默认使用http（FastMCP 2.3.0+推荐）
            config["transport"] = "http"
        
        logger.debug(f"Auto-inferred transport '{config['transport']}' for URL: {url}")
        return config

    @classmethod
    def _process_auth(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """处理认证信息"""
        config = deepcopy(config)
        
        # 处理Bearer token
        if "token" in config:
            if "headers" not in config:
                config["headers"] = {}
            config["headers"]["Authorization"] = f"Bearer {config['token'] }"
            del config["token"]
        
        # 处理API key
        if "api_key" in config:
            if "headers" not in config:
                config["headers"] = {}
            config["headers"]["X-API-Key"] = config["api_key"]
            del config["api_key"]
        
        return config

    @classmethod
    def _process_env_vars(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """处理环境变量"""
        config = deepcopy(config)
        
        # 确保env是字典格式
        if "env" in config and not isinstance(config["env"], dict):
            logger.warning("Invalid env format, removing")
            del config["env"]
        
        return config

    @classmethod
    def validate_and_suggest(cls, user_input: Union[str, List[str], Dict[str, Any]]) -> tuple[bool, str, Dict[str, Any]]:
        """
        验证用户输入并提供建议
        
        Returns:
            (是否有效, 错误/建议信息, 标准化后的配置)
        """
        try:
            # 标准化输入
            normalized_config = cls.normalize_user_input(user_input)
            
            # 验证标准化后的配置
            is_valid, message = cls._validate_normalized_config(normalized_config)
            
            if is_valid:
                return True, "Configuration is valid", normalized_config
            else:
                return False, message, normalized_config
                
        except Exception as e:
            return False, f"Configuration processing error: {e}", {"mcpServers": {}}

    @classmethod
    def _validate_normalized_config(cls, config: Dict[str, Any]) -> tuple[bool, str]:
        """验证标准化后的配置"""
        if not config.get("mcpServers"):
            return False, "No services found in configuration"
        
        issues = []
        for service_name, service_config in config["mcpServers"].items():
            if not isinstance(service_config, dict):
                issues.append(f"Service '{service_name}' has invalid configuration")
                continue
            
            has_url = "url" in service_config
            has_command = "command" in service_config
            
            if not has_url and not has_command:
                issues.append(f"Service '{service_name}' missing both 'url' and 'command'")
            elif has_url and has_command:
                issues.append(f"Service '{service_name}' has both 'url' and 'command' (conflicting)")
        
        if issues:
            return False, "; ".join(issues)
        
        return True, "All services are properly configured"

    @classmethod
    def get_user_friendly_error(cls, error: str) -> str:
        """将错误转换为用户友好的信息"""
        error_lower = error.lower()
        
        # 配置相关错误
        if "missing" in error_lower and ("url" in error_lower or "command" in error_lower):
            return "Service configuration incomplete. Each service needs either a 'url' (for remote services) or 'command' (for local services)."
        
        if "conflicting" in error_lower:
            return "Service configuration conflict. A service cannot have both 'url' and 'command' fields."
        
        # 网络相关错误
        if "connection" in error_lower:
            return "Cannot connect to the service. Please check if the service is running and the URL is correct."
        
        if "timeout" in error_lower:
            return "Service connection timeout. The service may be slow or unreachable."
        
        # 文件相关错误
        if "file not found" in error_lower or "no such file" in error_lower:
            return "Command file not found. Please check if the command path is correct and the file exists."
        
        if "permission" in error_lower:
            return "Permission denied. Please check if you have the necessary permissions to run the command."
        
        return error

