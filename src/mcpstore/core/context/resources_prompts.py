"""
MCPStore Resources and Prompts Module
Implementation of Resources and Prompts functionality
"""

import logging
from typing import Dict, List, Optional, Any, Union

from .types import ContextType

logger = logging.getLogger(__name__)

class ResourcesPromptsMixin:
    """Resources and Prompts mixin class"""
    
    def list_changed_tools(
        self,
        service_name: Optional[str] = None,
        force_refresh: bool = False
    ) -> Dict[str, Any]:
        """
        Tool change detection and processing method (synchronous version)

        Supports hybrid tool change detection with FastMCP notification mechanism + polling backup strategy

        Args:
            service_name: Specific service name (optional, None means check all services)
            force_refresh: Whether to force refresh (ignore cache and time intervals)

        Returns:
            Dict: Response containing change information
            {
                "changed": bool,           # Whether there are changes
                "services": List[str],     # List of services with changes
                "trigger": str,           # Trigger method: "notification" | "polling" | "manual"
                "timestamp": str,         # Detection time
                "details": Dict           # Detailed change information
            }
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return self._store.orchestrator.list_changed_tools(
            service_name=service_name,
            client_id=client_id,
            force_refresh=force_refresh
        )

    async def list_changed_tools_async(
        self,
        service_name: Optional[str] = None,
        force_refresh: bool = False
    ) -> Dict[str, Any]:
        """
        工具变更检测和处理方法（异步版本）

        Args:
            service_name: 特定服务名（可选，None表示检查所有服务）
            force_refresh: 是否强制刷新（忽略缓存和时间间隔）

        Returns:
            Dict: 包含变更信息的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.list_changed_tools_async(
            service_name=service_name,
            client_id=client_id,
            force_refresh=force_refresh
        )

    def list_resources(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的资源（同步版本）

        支持列出静态资源和基于模板的动态资源

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的资源）

        Returns:
            Dict: 包含资源列表的响应
            {
                "success": bool,           # 操作是否成功
                "resources": List[Dict],   # 资源列表
                "service_name": str,      # 服务名（如果指定）
                "timestamp": str,         # 操作时间
                "resource_count": int     # 资源数量
            }
        """
        return self._sync_helper.run_async(
            self.list_resources_async(service_name)
        )

    async def list_resources_async(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的资源（异步版本）

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的资源）

        Returns:
            Dict: 包含资源列表的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.list_resources_async(
            service_name=service_name,
            client_id=client_id
        )

    def list_resource_templates(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的资源模板（同步版本）

        支持列出动态资源的模板信息

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的资源模板）

        Returns:
            Dict: 包含资源模板列表的响应
            {
                "success": bool,           # 操作是否成功
                "templates": List[Dict],   # 资源模板列表
                "service_name": str,      # 服务名（如果指定）
                "timestamp": str,         # 操作时间
                "template_count": int     # 模板数量
            }
        """
        return self._sync_helper.run_async(
            self.list_resource_templates_async(service_name)
        )

    async def list_resource_templates_async(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的资源模板（异步版本）

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的资源模板）

        Returns:
            Dict: 包含资源模板列表的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.list_resource_templates_async(
            service_name=service_name,
            client_id=client_id
        )

    def read_resource(self, uri: str, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        读取资源内容（同步版本）

        支持读取静态资源和基于模板的动态资源

        Args:
            uri: 资源URI（如 "resource://config" 或 "weather://london/current"）
            service_name: 特定服务名（可选，None表示从所有服务中查找）

        Returns:
            Dict: 包含资源内容的响应
            {
                "success": bool,           # 操作是否成功
                "data": List[Dict],        # 资源内容列表
                "uri": str,               # 资源URI
                "service_name": str,      # 提供资源的服务名
                "timestamp": str,         # 操作时间
                "content_count": int      # 内容块数量
            }
        """
        return self._sync_helper.run_async(
            self.read_resource_async(uri, service_name)
        )

    async def read_resource_async(self, uri: str, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        读取资源内容（异步版本）

        Args:
            uri: 资源URI
            service_name: 特定服务名（可选）

        Returns:
            Dict: 包含资源内容的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.read_resource_async(
            uri=uri,
            service_name=service_name,
            client_id=client_id
        )

    def list_prompts(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的提示词（同步版本）

        支持列出所有可用的提示词模板

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的提示词）

        Returns:
            Dict: 包含提示词列表的响应
            {
                "success": bool,           # 操作是否成功
                "prompts": List[Dict],     # 提示词列表
                "service_name": str,      # 服务名（如果指定）
                "timestamp": str,         # 操作时间
                "prompt_count": int       # 提示词数量
            }
        """
        return self._sync_helper.run_async(
            self.list_prompts_async(service_name)
        )

    async def list_prompts_async(self, service_name: Optional[str] = None) -> Dict[str, Any]:
        """
        列出可用的提示词（异步版本）

        Args:
            service_name: 特定服务名（可选，None表示列出所有服务的提示词）

        Returns:
            Dict: 包含提示词列表的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.list_prompts_async(
            service_name=service_name,
            client_id=client_id
        )

    def get_prompt(
        self,
        name: str,
        arguments: Optional[Dict[str, Any]] = None,
        service_name: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        获取提示词内容（同步版本）

        支持获取带参数的动态提示词

        Args:
            name: 提示词名称
            arguments: 提示词参数（可选）
            service_name: 特定服务名（可选，None表示从所有服务中查找）

        Returns:
            Dict: 包含提示词内容的响应
            {
                "success": bool,           # 操作是否成功
                "prompt": Dict,           # 提示词内容
                "name": str,              # 提示词名称
                "service_name": str,      # 提供提示词的服务名
                "timestamp": str,         # 操作时间
                "arguments": Dict         # 使用的参数
            }
        """
        return self._sync_helper.run_async(
            self.get_prompt_async(name, arguments, service_name)
        )

    async def get_prompt_async(
        self,
        name: str,
        arguments: Optional[Dict[str, Any]] = None,
        service_name: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        获取提示词内容（异步版本）

        Args:
            name: 提示词名称
            arguments: 提示词参数（可选）
            service_name: 特定服务名（可选）

        Returns:
            Dict: 包含提示词内容的响应
        """
        client_id = None
        if self._context_type == ContextType.AGENT:
            client_id = self._agent_id

        return await self._store.orchestrator.get_prompt_async(
            name=name,
            arguments=arguments,
            service_name=service_name,
            client_id=client_id
        )
