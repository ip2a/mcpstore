#!/usr/bin/env python3
"""
OpenAPI 深度集成
自动化 API 转换，自定义路由映射，智能生成 MCP 组件名称
"""

import logging
import re
import httpx
from typing import Dict, List, Any, Optional, Union, Tuple
from dataclasses import dataclass, field
from enum import Enum
from urllib.parse import urlparse
import json

logger = logging.getLogger(__name__)

class MCPComponentType(Enum):
    """MCP 组件类型"""
    TOOL = "tool"
    RESOURCE = "resource"
    RESOURCE_TEMPLATE = "resource_template"

class HTTPMethod(Enum):
    """HTTP 方法"""
    GET = "GET"
    POST = "POST"
    PUT = "PUT"
    DELETE = "DELETE"
    PATCH = "PATCH"
    HEAD = "HEAD"
    OPTIONS = "OPTIONS"

@dataclass
class RouteMapping:
    """路由映射配置"""
    path_pattern: str                    # 路径模式，支持正则表达式
    method: Optional[HTTPMethod] = None  # HTTP 方法，None 表示匹配所有方法
    mcp_type: MCPComponentType = MCPComponentType.TOOL  # 映射到的 MCP 组件类型
    name_template: Optional[str] = None  # 名称模板
    description_template: Optional[str] = None  # 描述模板
    tags: List[str] = field(default_factory=list)  # 标签

@dataclass
class OpenAPIServiceConfig:
    """OpenAPI 服务配置"""
    name: str
    spec_url: str
    base_url: Optional[str] = None
    auth_config: Optional[Dict[str, Any]] = None
    route_mappings: List[RouteMapping] = field(default_factory=list)
    custom_names: Dict[str, str] = field(default_factory=dict)  # operation_id -> custom_name
    global_tags: List[str] = field(default_factory=list)
    auto_sync: bool = False  # 是否自动同步 API 变更

class OpenAPIAnalyzer:
    """OpenAPI 规范分析器"""
    
    def __init__(self):
        self._spec_cache: Dict[str, Dict[str, Any]] = {}
    
    async def fetch_spec(self, spec_url: str) -> Dict[str, Any]:
        """获取 OpenAPI 规范"""
        if spec_url in self._spec_cache:
            return self._spec_cache[spec_url]
        
        try:
            async with httpx.AsyncClient() as client:
                response = await client.get(spec_url)
                response.raise_for_status()
                spec = response.json()
                self._spec_cache[spec_url] = spec
                logger.info(f"Fetched OpenAPI spec from {spec_url}")
                return spec
        except Exception as e:
            logger.error(f"Failed to fetch OpenAPI spec from {spec_url}: {e}")
            raise
    
    def analyze_endpoints(self, spec: Dict[str, Any]) -> List[Dict[str, Any]]:
        """分析 API 端点"""
        endpoints = []
        paths = spec.get("paths", {})
        
        for path, path_item in paths.items():
            for method, operation in path_item.items():
                if method.upper() not in [m.value for m in HTTPMethod]:
                    continue
                
                endpoint_info = {
                    "path": path,
                    "method": method.upper(),
                    "operation_id": operation.get("operationId"),
                    "summary": operation.get("summary"),
                    "description": operation.get("description"),
                    "tags": operation.get("tags", []),
                    "parameters": operation.get("parameters", []),
                    "request_body": operation.get("requestBody"),
                    "responses": operation.get("responses", {}),
                    "security": operation.get("security", [])
                }
                endpoints.append(endpoint_info)
        
        return endpoints
    
    def suggest_mcp_type(self, endpoint: Dict[str, Any]) -> MCPComponentType:
        """建议 MCP 组件类型"""
        method = endpoint["method"]
        path = endpoint["path"]
        
        # GET 请求通常映射为 Resource
        if method == "GET":
            # 如果路径包含参数，映射为 ResourceTemplate
            if "{" in path and "}" in path:
                return MCPComponentType.RESOURCE_TEMPLATE
            else:
                return MCPComponentType.RESOURCE
        
        # 其他方法映射为 Tool
        return MCPComponentType.TOOL
    
    def generate_component_name(self, endpoint: Dict[str, Any], custom_names: Dict[str, str] = None) -> str:
        """生成组件名称"""
        operation_id = endpoint.get("operation_id")
        
        # 使用自定义名称
        if custom_names and operation_id and operation_id in custom_names:
            return custom_names[operation_id]
        
        # 使用 operation_id（截断到第一个双下划线）
        if operation_id:
            name = operation_id.split("__")[0]
            return self._slugify_name(name)
        
        # 根据路径和方法生成名称
        method = endpoint["method"].lower()
        path = endpoint["path"]
        
        # 清理路径
        path_parts = [part for part in path.split("/") if part and not part.startswith("{")]
        if path_parts:
            resource = "_".join(path_parts)
        else:
            resource = "api"
        
        name = f"{method}_{resource}"
        return self._slugify_name(name)
    
    def _slugify_name(self, name: str) -> str:
        """将名称转换为合法的标识符"""
        # 转换为小写
        name = name.lower()
        # 替换特殊字符为下划线
        name = re.sub(r'[^a-z0-9_]', '_', name)
        # 移除连续的下划线
        name = re.sub(r'_+', '_', name)
        # 移除开头和结尾的下划线
        name = name.strip('_')
        # 限制长度
        if len(name) > 56:
            name = name[:56].rstrip('_')
        
        return name or "unnamed"

class RouteMapper:
    """路由映射器"""
    
    def __init__(self):
        self._default_mappings = self._create_default_mappings()
    
    def _create_default_mappings(self) -> List[RouteMapping]:
        """创建默认路由映射"""
        return [
            # GET 请求映射为 Resource
            RouteMapping(
                path_pattern=r".*",
                method=HTTPMethod.GET,
                mcp_type=MCPComponentType.RESOURCE,
                tags=["read-only"]
            ),
            # POST/PUT/DELETE 映射为 Tool
            RouteMapping(
                path_pattern=r".*",
                method=HTTPMethod.POST,
                mcp_type=MCPComponentType.TOOL,
                tags=["write"]
            ),
            RouteMapping(
                path_pattern=r".*",
                method=HTTPMethod.PUT,
                mcp_type=MCPComponentType.TOOL,
                tags=["write", "update"]
            ),
            RouteMapping(
                path_pattern=r".*",
                method=HTTPMethod.DELETE,
                mcp_type=MCPComponentType.TOOL,
                tags=["write", "delete", "destructive"]
            )
        ]
    
    def apply_mappings(self, endpoint: Dict[str, Any], custom_mappings: List[RouteMapping] = None) -> Tuple[MCPComponentType, List[str]]:
        """应用路由映射"""
        mappings = custom_mappings or self._default_mappings
        path = endpoint["path"]
        method = HTTPMethod(endpoint["method"])
        
        for mapping in mappings:
            # 检查路径模式
            if not re.match(mapping.path_pattern, path):
                continue
            
            # 检查方法
            if mapping.method and mapping.method != method:
                continue
            
            # 匹配成功
            return mapping.mcp_type, mapping.tags
        
        # 默认映射
        if method == HTTPMethod.GET:
            return MCPComponentType.RESOURCE, ["read-only"]
        else:
            return MCPComponentType.TOOL, ["write"]

class OpenAPIIntegrationManager:
    """OpenAPI 集成管理器"""
    
    def __init__(self):
        self.analyzer = OpenAPIAnalyzer()
        self.route_mapper = RouteMapper()
        self._services: Dict[str, OpenAPIServiceConfig] = {}
    
    def register_openapi_service(self, config: OpenAPIServiceConfig):
        """注册 OpenAPI 服务"""
        self._services[config.name] = config
        logger.info(f"Registered OpenAPI service: {config.name}")
    
    async def import_openapi_service(
        self,
        name: str,
        spec_url: str,
        base_url: Optional[str] = None,
        route_mappings: List[RouteMapping] = None,
        custom_names: Dict[str, str] = None
    ) -> Dict[str, Any]:
        """导入 OpenAPI 服务"""
        
        # 获取规范
        spec = await self.analyzer.fetch_spec(spec_url)
        
        # 分析端点
        endpoints = self.analyzer.analyze_endpoints(spec)
        
        # 生成 MCP 组件
        components = []
        for endpoint in endpoints:
            # 应用路由映射
            mcp_type, tags = self.route_mapper.apply_mappings(endpoint, route_mappings)
            
            # 生成组件名称
            component_name = self.analyzer.generate_component_name(endpoint, custom_names)
            
            component = {
                "name": component_name,
                "type": mcp_type.value,
                "endpoint": endpoint,
                "tags": tags + (endpoint.get("tags", [])),
                "description": endpoint.get("description") or endpoint.get("summary"),
                "service_name": name
            }
            components.append(component)
        
        # 创建服务配置
        service_config = OpenAPIServiceConfig(
            name=name,
            spec_url=spec_url,
            base_url=base_url or self._extract_base_url(spec),
            route_mappings=route_mappings or [],
            custom_names=custom_names or {}
        )
        self.register_openapi_service(service_config)
        
        result = {
            "service_name": name,
            "spec_info": {
                "title": spec.get("info", {}).get("title"),
                "version": spec.get("info", {}).get("version"),
                "description": spec.get("info", {}).get("description")
            },
            "components": components,
            "total_endpoints": len(endpoints),
            "component_types": {
                "tools": len([c for c in components if c["type"] == "tool"]),
                "resources": len([c for c in components if c["type"] == "resource"]),
                "resource_templates": len([c for c in components if c["type"] == "resource_template"])
            }
        }
        
        logger.info(f"Imported OpenAPI service {name}: {len(components)} components generated")
        return result
    
    async def sync_service_changes(self, service_name: str) -> Dict[str, Any]:
        """同步服务变更"""
        if service_name not in self._services:
            raise ValueError(f"Service {service_name} not found")
        
        config = self._services[service_name]
        
        # 重新获取规范
        new_spec = await self.analyzer.fetch_spec(config.spec_url)
        new_endpoints = self.analyzer.analyze_endpoints(new_spec)
        
        # 比较变更
        # 这里可以实现更复杂的变更检测逻辑
        
        return {
            "service_name": service_name,
            "changes_detected": True,  # 简化实现
            "new_endpoints_count": len(new_endpoints)
        }
    
    def create_custom_route_mapping(
        self,
        service_name: str,
        path_patterns: Dict[str, MCPComponentType]
    ) -> List[RouteMapping]:
        """创建自定义路由映射"""
        mappings = []
        for pattern, mcp_type in path_patterns.items():
            mapping = RouteMapping(
                path_pattern=pattern,
                mcp_type=mcp_type,
                tags=["custom-mapped"]
            )
            mappings.append(mapping)
        
        return mappings
    
    def get_service_info(self, service_name: str) -> Optional[OpenAPIServiceConfig]:
        """获取服务信息"""
        return self._services.get(service_name)
    
    def list_services(self) -> List[str]:
        """列出所有服务"""
        return list(self._services.keys())
    
    def _extract_base_url(self, spec: Dict[str, Any]) -> Optional[str]:
        """从规范中提取基础 URL"""
        servers = spec.get("servers", [])
        if servers:
            return servers[0].get("url")
        return None

# 全局实例
_global_openapi_manager = None

def get_openapi_manager() -> OpenAPIIntegrationManager:
    """获取全局 OpenAPI 集成管理器"""
    global _global_openapi_manager
    if _global_openapi_manager is None:
        _global_openapi_manager = OpenAPIIntegrationManager()
    return _global_openapi_manager
