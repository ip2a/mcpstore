#!/usr/bin/env python3
"""
OpenAPI Deep Integration
Automated API conversion, custom route mapping, intelligent MCP component name generation
"""

import logging
import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Any, Optional, Tuple

import httpx

logger = logging.getLogger(__name__)

class MCPComponentType(Enum):
    """MCP component types"""
    TOOL = "tool"
    RESOURCE = "resource"
    RESOURCE_TEMPLATE = "resource_template"

class HTTPMethod(Enum):
    """HTTP methods"""
    GET = "GET"
    POST = "POST"
    PUT = "PUT"
    DELETE = "DELETE"
    PATCH = "PATCH"
    HEAD = "HEAD"
    OPTIONS = "OPTIONS"

@dataclass
class RouteMapping:
    """Route mapping configuration"""
    path_pattern: str                    # Path pattern, supports regular expressions
    method: Optional[HTTPMethod] = None  # HTTP method, None means match all methods
    mcp_type: MCPComponentType = MCPComponentType.TOOL  # MCP component type to map to
    name_template: Optional[str] = None  # Name template
    description_template: Optional[str] = None  # Description template
    tags: List[str] = field(default_factory=list)  # Tags

@dataclass
class OpenAPIServiceConfig:
    """OpenAPI service configuration"""
    name: str
    spec_url: str
    base_url: Optional[str] = None
    auth_config: Optional[Dict[str, Any]] = None
    route_mappings: List[RouteMapping] = field(default_factory=list)
    custom_names: Dict[str, str] = field(default_factory=dict)  # operation_id -> custom_name
    global_tags: List[str] = field(default_factory=list)
    auto_sync: bool = False  # Whether to auto-sync API changes

class OpenAPIAnalyzer:
    """OpenAPI Specification Analyzer"""
    
    def __init__(self):
        self._spec_cache: Dict[str, Dict[str, Any]] = {}
    
    async def fetch_spec(self, spec_url: str) -> Dict[str, Any]:
        """Get OpenAPI specification"""
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
        """Analyze API endpoints"""
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
        """Suggest MCP component type"""
        method = endpoint["method"]
        path = endpoint["path"]
        
        # GET requests usually map to Resource
        if method == "GET":
            # If path contains parameters, map to ResourceTemplate
            if "{" in path and "}" in path:
                return MCPComponentType.RESOURCE_TEMPLATE
            else:
                return MCPComponentType.RESOURCE
        
        # Other methods map to Tool
        return MCPComponentType.TOOL
    
    def generate_component_name(self, endpoint: Dict[str, Any], custom_names: Dict[str, str] = None) -> str:
        """Generate MCP component name"""
        if custom_names and endpoint.get("operation_id") in custom_names:
            return custom_names[endpoint["operation_id"]]
        
        # Prioritize using operationId
        operation_id = endpoint.get("operation_id")
        if operation_id:
            name = operation_id
        else:
            # Otherwise use method + path combination
            method = endpoint.get("method", "GET").lower()
            path = endpoint.get("path", "/")
            name = f"{method}_{path.strip('/').replace('/', '_')}"
        
        # Clean up invalid characters
        name = re.sub(r"[^a-zA-Z0-9_]+", "_", name)
        name = re.sub(r"_+", "_", name).strip("_")
        
        return name

class RouteMapper:
    """Route Mapper"""
    
    def apply_mappings(self, endpoint: Dict[str, Any], mappings: List[RouteMapping] = None) -> Tuple[MCPComponentType, List[str]]:
        """Apply route mappings, return (MCP component type, tag list)"""
        if not mappings:
            # No mapping configured, use default suggestion
            return OpenAPIAnalyzer().suggest_mcp_type(endpoint), endpoint.get("tags", [])
        
        for mapping in mappings:
            if self._match_endpoint(endpoint, mapping):
                return mapping.mcp_type, mapping.tags + endpoint.get("tags", [])
        
        return OpenAPIAnalyzer().suggest_mcp_type(endpoint), endpoint.get("tags", [])
    
    def _match_endpoint(self, endpoint: Dict[str, Any], mapping: RouteMapping) -> bool:
        """Determine if endpoint matches mapping rule"""
        path_match = re.match(mapping.path_pattern, endpoint["path"]) is not None
        method_match = (mapping.method is None) or (endpoint["method"] == mapping.method.value)
        return path_match and method_match

class OpenAPIIntegrationManager:
    """OpenAPI Integration Manager"""
    
    def __init__(self):
        self.analyzer = OpenAPIAnalyzer()
        self.route_mapper = RouteMapper()
        self._services: Dict[str, OpenAPIServiceConfig] = {}
    
    def register_openapi_service(self, config: OpenAPIServiceConfig):
        """Register OpenAPI service"""
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
        """Import OpenAPI service"""
        
        # Get specification
        spec = await self.analyzer.fetch_spec(spec_url)

        # Analyze endpoints
        endpoints = self.analyzer.analyze_endpoints(spec)
        
        # Generate MCP components
        components = []
        for endpoint in endpoints:
            # Apply route mappings
            mcp_type, tags = self.route_mapper.apply_mappings(endpoint, route_mappings)
            
            # Generate component name
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
        
        # Create service configuration
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
        """Sync service changes"""
        if service_name not in self._services:
            raise ValueError(f"Service {service_name} not found")
        
        config = self._services[service_name]
        
        # Re-fetch specification
        new_spec = await self.analyzer.fetch_spec(config.spec_url)
        new_endpoints = self.analyzer.analyze_endpoints(new_spec)
        
        # Compare changes (simplified)
        return {
            "service_name": service_name,
            "changes_detected": True,
            "new_endpoints_count": len(new_endpoints)
        }
    
    def _extract_base_url(self, spec: Dict[str, Any]) -> str:
        """Extract base URL from OpenAPI specification"""
        servers = spec.get("servers", [])
        if servers and isinstance(servers, list) and servers[0].get("url"):
            return servers[0]["url"]
        return ""

# Global instance
_global_openapi_manager = None

def get_openapi_manager() -> OpenAPIIntegrationManager:
    """Get global OpenAPI integration manager"""
    global _global_openapi_manager
    if _global_openapi_manager is None:
        _global_openapi_manager = OpenAPIIntegrationManager()
    return _global_openapi_manager

