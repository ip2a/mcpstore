#!/usr/bin/env python3
"""
Configuration Processor - handles conversion between user configuration and FastMCP configuration
Lenient to users, strict to FastMCP
"""

import logging
from copy import deepcopy
from typing import Dict, Any

logger = logging.getLogger(__name__)

class ConfigProcessor:
    """
    Configuration Processor: handles conversion between user configuration and FastMCP configuration

    Design philosophy:
    1. Lenient to users: allow extra fields, transport optional
    2. Strict to FastMCP: ensure format fully complies with requirements
    3. Intelligent inference: automatically handle transport field
    """
    
    # Standard fields supported by FastMCP
    FASTMCP_REMOTE_FIELDS = {
        "url", "transport", "headers", "timeout", "keep_alive"
    }
    
    FASTMCP_LOCAL_FIELDS = {
        "command", "args", "env", "working_dir", "timeout"
    }
    
    # Supported transport types
    VALID_TRANSPORTS = {
        "streamable-http", "sse", "stdio"
    }
    
    @classmethod
    def process_user_config_for_fastmcp(cls, user_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        Convert user configuration to FastMCP-compatible configuration

        Args:
            user_config: User's original configuration

        Returns:
            FastMCP-compatible configuration
        """
        if not isinstance(user_config, dict) or "mcpServers" not in user_config:
            logger.warning("Invalid config format, returning as-is")
            return user_config

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
                # 提供更详细的错误信息
                if "missing" in str(e).lower():
                    logger.warning(f"Service '{service_name}' has missing required fields - removing from FastMCP config")
                elif "url" in str(e).lower() and "command" in str(e).lower():
                    logger.warning(f"Service '{service_name}' has conflicting url/command fields - removing from FastMCP config")
                else:
                    logger.warning(f"Service '{service_name}' has configuration errors - removing from FastMCP config: {e}")

                services_to_remove.append(service_name)
                continue

        # 移除有问题的服务
        for service_name in services_to_remove:
            del fastmcp_config["mcpServers"][service_name]

        return fastmcp_config
    
    @classmethod
    def _process_single_service(cls, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        处理单个服务配置
        
        Args:
            service_config: 单个服务的配置
            
        Returns:
            处理后的服务配置
        """
        if not isinstance(service_config, dict):
            return service_config
        
        # 深拷贝避免修改原配置
        processed = deepcopy(service_config)
        
        # 判断服务类型
        if "url" in processed:
            # 远程服务
            processed = cls._process_remote_service(processed)
        elif "command" in processed:
            # 本地服务
            processed = cls._process_local_service(processed)
        else:
            logger.warning("Service config missing both 'url' and 'command', keeping as-is")
        
        return processed
    
    @classmethod
    def _process_remote_service(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        处理远程服务配置
        
        Args:
            config: 远程服务配置
            
        Returns:
            处理后的配置
        """
        # 1. 智能推断transport字段
        config = cls._infer_transport(config)
        
        # 2. 清理非FastMCP字段（保留用户自定义字段在日志中）
        user_fields = set(config.keys()) - cls.FASTMCP_REMOTE_FIELDS
        if user_fields:
            logger.debug(f"Removing user-defined fields for FastMCP: {user_fields}")
        
        # 3. 只保留FastMCP支持的字段
        fastmcp_config = {
            key: value for key, value in config.items() 
            if key in cls.FASTMCP_REMOTE_FIELDS
        }
        
        # 4. 确保必要字段存在
        if "url" not in fastmcp_config:
            raise ValueError("Remote service missing required 'url' field")
        
        return fastmcp_config
    
    @classmethod
    def _process_local_service(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        处理本地服务配置
        
        Args:
            config: 本地服务配置
            
        Returns:
            处理后的配置
        """
        # 1. 移除transport字段（本地服务不需要）
        if "transport" in config:
            logger.debug("Removing 'transport' field from local service (not needed)")
            config = deepcopy(config)
            del config["transport"]
        
        # 2. 清理非FastMCP字段
        user_fields = set(config.keys()) - cls.FASTMCP_LOCAL_FIELDS
        if user_fields:
            logger.debug(f"Removing user-defined fields for FastMCP: {user_fields}")
        
        # 3. 只保留FastMCP支持的字段
        fastmcp_config = {
            key: value for key, value in config.items() 
            if key in cls.FASTMCP_LOCAL_FIELDS
        }
        
        # 4. 确保必要字段存在
        if "command" not in fastmcp_config:
            raise ValueError("Local service missing required 'command' field")
        
        return fastmcp_config
    
    @classmethod
    def _infer_transport(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        智能推断transport字段
        
        Args:
            config: 服务配置
            
        Returns:
            包含正确transport字段的配置
        """
        config = deepcopy(config)
        url = config.get("url", "")
        
        # 如果用户已经指定了transport，验证并保留
        if "transport" in config:
            transport = config["transport"]
            if transport in cls.VALID_TRANSPORTS:
                logger.debug(f"Using user-specified transport: {transport}")
                return config
            else:
                logger.warning(f"Invalid transport '{transport}', will auto-infer")
                del config["transport"]
        
        # 自动推断transport
        if "/sse" in url.lower():
            # URL包含/sse，使用SSE传输
            config["transport"] = "sse"
            logger.debug(f"Auto-inferred transport 'sse' from URL: {url}")
        else:
            # 默认使用streamable-http
            config["transport"] = "streamable-http"
            logger.debug(f"Auto-inferred transport 'streamable-http' for URL: {url}")
        
        return config
    
    @classmethod
    def validate_user_config(cls, config: Dict[str, Any]) -> tuple[bool, str]:
        """
        验证用户配置的基本有效性（宽松验证）
        
        Args:
            config: 用户配置
            
        Returns:
            (是否有效, 错误信息)
        """
        try:
            # 1. 检查基本结构
            if not isinstance(config, dict):
                return False, "Config must be a dictionary"
            
            if "mcpServers" not in config:
                return False, "Config missing 'mcpServers' field"
            
            if not isinstance(config["mcpServers"], dict):
                return False, "'mcpServers' must be a dictionary"
            
            # 2. 检查每个服务
            for service_name, service_config in config["mcpServers"].items():
                if not isinstance(service_config, dict):
                    return False, f"Service '{service_name}' config must be a dictionary"
                
                # 检查必要字段
                has_url = "url" in service_config
                has_command = "command" in service_config
                
                if not has_url and not has_command:
                    return False, f"Service '{service_name}' missing both 'url' and 'command' fields"
                
                if has_url and has_command:
                    return False, f"Service '{service_name}' cannot have both 'url' and 'command' fields"
            
            return True, "Config is valid"
            
        except Exception as e:
            return False, f"Config validation error: {e}"
    
    @classmethod
    def get_user_friendly_error(cls, fastmcp_error: str) -> str:
        """
        将FastMCP错误转换为用户友好的错误信息

        Args:
            fastmcp_error: FastMCP的原始错误信息

        Returns:
            用户友好的错误信息
        """
        error_lower = fastmcp_error.lower()

        # 配置验证错误
        if "validation errors" in error_lower:
            return "Service configuration has validation errors. This may be due to user-defined fields that are not supported by FastMCP."

        if "field required" in error_lower:
            return "Missing required field. Please ensure your service has either 'url' or 'command' field."

        if "extra inputs are not permitted" in error_lower:
            return "Configuration contains unsupported fields. MCPStore will automatically filter these for FastMCP compatibility."

        if "input should be" in error_lower:
            return "Invalid field value. Please check your service configuration format."

        # 网络相关错误
        if "getaddrinfo failed" in error_lower:
            return "Cannot resolve the service URL. Please check the URL and network connection."

        if "connection refused" in error_lower:
            return "Connection refused. Please verify the service is running and accessible."

        if "connection closed" in error_lower:
            return "Connection was closed by the service. The service may not be ready or may have crashed."

        if "timeout" in error_lower:
            return "Connection timeout. The service may be slow to respond or unreachable."

        if "connection" in error_lower:
            return "Connection failed. Please verify the service is running and accessible."

        # 文件系统相关错误
        if "no such file" in error_lower or "file not found" in error_lower:
            return "Required file not found. Please ensure all command files exist and are accessible."

        if "permission denied" in error_lower or "access denied" in error_lower:
            return "Permission denied. Please check file permissions and execution rights."

        # 返回原始错误（已经足够友好的情况）
        return fastmcp_error

