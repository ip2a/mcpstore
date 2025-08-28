"""
MCPOrchestrator Resources and Prompts Module
Resources/Prompts模块 - 包含FastMCP的Resources和Prompts功能支持
"""

import time
import logging
from typing import Dict, List, Any, Optional

logger = logging.getLogger(__name__)

class ResourcesPromptsMixin:
    """Resources/Prompts混入类"""

    # === 工具变更检测接口 ===

    def list_changed_tools(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None,
        force_refresh: bool = False
    ) -> Dict[str, Any]:
        """
        工具变更检测和处理方法（同步版本）

        Args:
            service_name: 特定服务名（可选，None表示检查所有服务）
            client_id: 特定客户端ID（可选）
            force_refresh: 是否强制刷新（忽略缓存和时间间隔）

        Returns:
            Dict: 包含变更信息的响应
            {
                "changed": bool,           # 是否有变更
                "services": List[str],     # 发生变更的服务列表
                "trigger": str,           # 触发方式："notification" | "polling" | "manual"
                "timestamp": str,         # 检测时间
                "details": Dict           # 详细变更信息
            }
        """
        if self.tools_update_monitor:
            return self.tools_update_monitor.list_changed_tools(
                service_name=service_name,
                client_id=client_id,
                force_refresh=force_refresh,
                trigger="manual"
            )
        else:
            logger.warning("ToolsUpdateMonitor not available")
            return {
                "changed": False,
                "services": [],
                "trigger": "manual",
                "timestamp": self._get_timestamp(),
                "details": {"error": "ToolsUpdateMonitor not available"}
            }

    async def list_changed_tools_async(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None,
        force_refresh: bool = False,
        trigger: str = "manual"
    ) -> Dict[str, Any]:
        """
        工具变更检测和处理方法（异步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）
            force_refresh: 是否强制刷新
            trigger: 触发方式

        Returns:
            Dict: 包含变更信息的响应
        """
        if self.tools_update_monitor:
            return await self.tools_update_monitor.list_changed_tools_async(
                service_name=service_name,
                client_id=client_id,
                force_refresh=force_refresh,
                trigger=trigger
            )
        else:
            logger.warning("ToolsUpdateMonitor not available")
            return {
                "changed": False,
                "services": [],
                "trigger": trigger,
                "timestamp": self._get_timestamp(),
                "details": {"error": "ToolsUpdateMonitor not available"}
            }

    # === Resources操作支持 ===

    def list_resources(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的资源（同步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源列表的响应
        """
        return self._sync_helper.run_async(
            self.list_resources_async(service_name, client_id)
        )

    async def list_resources_async(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的资源（异步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源列表的响应
        """
        try:
            if not client_id:
                client_id = self.client_manager.global_agent_store_id

            if service_name:
                # 获取特定服务的资源
                # 从Registry获取当前活跃会话
                client = self.registry.get_session(client_id, service_name)
                if not client:
                    return {
                        "success": False,
                        "error": f"Service '{service_name}' not found or not connected",
                        "data": [],
                        "service_name": service_name,
                        "timestamp": self._get_timestamp()
                    }

                resources = await client.list_resources()
                return {
                    "success": True,
                    "data": [self._safe_model_dump(resource) for resource in resources],
                    "service_name": service_name,
                    "timestamp": self._get_timestamp(),
                    "count": len(resources)
                }
            else:
                # 获取所有服务的资源
                all_resources = {}
                services = self.registry.get_services(client_id)

                for sname in services:
                    try:
                        client = self.client_manager.get_client(client_id, sname)
                        if client:
                            resources = await client.list_resources()
                            all_resources[sname] = [self._safe_model_dump(resource) for resource in resources]
                    except Exception as e:
                        logger.warning(f"Failed to get resources from service {sname}: {e}")
                        all_resources[sname] = []

                total_count = sum(len(resources) for resources in all_resources.values())
                return {
                    "success": True,
                    "data": all_resources,
                    "timestamp": self._get_timestamp(),
                    "total_count": total_count,
                    "services_count": len(all_resources)
                }

        except Exception as e:
            logger.error(f"Error listing resources: {e}")
            return {
                "success": False,
                "error": str(e),
                "data": [],
                "timestamp": self._get_timestamp()
            }

    def list_resource_templates(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的资源模板（同步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源模板列表的响应
        """
        return self._sync_helper.run_async(
            self.list_resource_templates_async(service_name, client_id)
        )

    async def list_resource_templates_async(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的资源模板（异步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源模板列表的响应
        """
        try:
            if not client_id:
                client_id = self.client_manager.global_agent_store_id

            if service_name:
                # 获取特定服务的资源模板
                client = self.client_manager.get_client(client_id, service_name)
                if not client:
                    return {
                        "success": False,
                        "error": f"Service '{service_name}' not found",
                        "data": [],
                        "service_name": service_name,
                        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                    }

                templates = await client.list_resource_templates()
                return {
                    "success": True,
                    "data": [self._safe_model_dump(template) for template in templates],
                    "service_name": service_name,
                    "timestamp": self._get_timestamp(),
                    "count": len(templates)
                }
            else:
                # 获取所有服务的资源模板
                all_templates = {}
                services = self.registry.get_services(client_id)

                for sname in services:
                    try:
                        client = self.client_manager.get_client(client_id, sname)
                        if client:
                            templates = await client.list_resource_templates()
                            all_templates[sname] = [template.model_dump() for template in templates]
                    except Exception as e:
                        logger.warning(f"Failed to get resource templates from service {sname}: {e}")
                        all_templates[sname] = []

                total_count = sum(len(templates) for templates in all_templates.values())
                return {
                    "success": True,
                    "data": all_templates,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                    "total_count": total_count,
                    "services_count": len(all_templates)
                }

        except Exception as e:
            logger.error(f"Error listing resource templates: {e}")
            return {
                "success": False,
                "error": str(e),
                "data": [],
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
            }

    def read_resource(
        self,
        uri: str,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        读取资源内容（同步版本）

        Args:
            uri: 资源URI
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源内容的响应
        """
        return self._sync_helper.run_async(
            self.read_resource_async(uri, service_name, client_id)
        )

    async def read_resource_async(
        self,
        uri: str,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        读取资源内容（异步版本）

        Args:
            uri: 资源URI
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含资源内容的响应
        """
        # 参数验证
        if not uri or not isinstance(uri, str):
            return {
                "success": False,
                "error": "Invalid URI parameter: URI must be a non-empty string",
                "data": None,
                "uri": uri,
                "timestamp": self._get_timestamp()
            }

        try:
            if not client_id:
                client_id = self.client_manager.global_agent_store_id

            if service_name:
                # 从特定服务读取资源
                client = self.client_manager.get_client(client_id, service_name)
                if not client:
                    return {
                        "success": False,
                        "error": f"Service '{service_name}' not found",
                        "data": None,
                        "uri": uri,
                        "service_name": service_name,
                        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                    }

                content = await client.read_resource(uri)
                return {
                    "success": True,
                    "data": [self._safe_model_dump(item) for item in content],
                    "uri": uri,
                    "service_name": service_name,
                    "timestamp": self._get_timestamp(),
                    "content_count": len(content)
                }
            else:
                # TODO: 权限控制 - 后续考虑添加资源访问权限验证
                # TODO: 缓存策略 - 后续考虑添加资源内容缓存

                # 尝试从所有服务读取资源（找到第一个匹配的）
                services = self.registry.get_services(client_id)
                last_error = None

                for sname in services:
                    try:
                        client = self.client_manager.get_client(client_id, sname)
                        if client:
                            content = await client.read_resource(uri)
                            return {
                                "success": True,
                                "data": [item.model_dump() for item in content],
                                "uri": uri,
                                "service_name": sname,
                                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                                "content_count": len(content)
                            }
                    except Exception as e:
                        last_error = e
                        continue

                return {
                    "success": False,
                    "error": f"Resource '{uri}' not found in any service. Last error: {last_error}",
                    "data": None,
                    "uri": uri,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                }

        except Exception as e:
            logger.error(f"Error reading resource {uri}: {e}")
            return {
                "success": False,
                "error": str(e),
                "data": None,
                "uri": uri,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
            }

    # === Prompts操作支持 ===

    def list_prompts(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的提示词（同步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含提示词列表的响应
        """
        return self._sync_helper.run_async(
            self.list_prompts_async(service_name, client_id)
        )

    async def list_prompts_async(
        self,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        列出可用的提示词（异步版本）

        Args:
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含提示词列表的响应
        """
        try:
            if not client_id:
                client_id = self.client_manager.global_agent_store_id

            if service_name:
                # 获取特定服务的提示词
                client = self.registry.get_session(client_id, service_name)
                if not client:
                    return {
                        "success": False,
                        "error": f"Service '{service_name}' not found or not connected",
                        "data": [],
                        "service_name": service_name,
                        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                    }

                prompts = await client.list_prompts()
                return {
                    "success": True,
                    "data": [prompt.model_dump() for prompt in prompts],
                    "service_name": service_name,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                    "count": len(prompts)
                }
            else:
                # 获取所有服务的提示词
                all_prompts = {}
                services = self.registry.get_services(client_id)

                for sname in services:
                    try:
                        client = self.client_manager.get_client(client_id, sname)
                        if client:
                            prompts = await client.list_prompts()
                            all_prompts[sname] = [prompt.model_dump() for prompt in prompts]
                    except Exception as e:
                        logger.warning(f"Failed to get prompts from service {sname}: {e}")
                        all_prompts[sname] = []

                total_count = sum(len(prompts) for prompts in all_prompts.values())
                return {
                    "success": True,
                    "data": all_prompts,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                    "total_count": total_count,
                    "services_count": len(all_prompts)
                }

        except Exception as e:
            logger.error(f"Error listing prompts: {e}")
            return {
                "success": False,
                "error": str(e),
                "data": [],
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
            }

    def get_prompt(
        self,
        name: str,
        arguments: Optional[Dict] = None,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        获取提示词内容（同步版本）

        Args:
            name: 提示词名称
            arguments: 提示词参数（可选）
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含提示词内容的响应
        """
        return self._sync_helper.run_async(
            self.get_prompt_async(name, arguments, service_name, client_id)
        )

    async def get_prompt_async(
        self,
        name: str,
        arguments: Optional[Dict] = None,
        service_name: Optional[str] = None,
        client_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        获取提示词内容（异步版本）

        Args:
            name: 提示词名称
            arguments: 提示词参数（可选）
            service_name: 特定服务名（可选）
            client_id: 特定客户端ID（可选）

        Returns:
            Dict: 包含提示词内容的响应
        """
        try:
            if not client_id:
                client_id = self.client_manager.global_agent_store_id

            if arguments is None:
                arguments = {}

            if service_name:
                # 从特定服务获取提示词
                client = self.client_manager.get_client(client_id, service_name)
                if not client:
                    return {
                        "success": False,
                        "error": f"Service '{service_name}' not found",
                        "data": None,
                        "name": name,
                        "service_name": service_name,
                        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                    }

                result = await client.get_prompt(name, arguments)
                return {
                    "success": True,
                    "data": result.model_dump(),
                    "name": name,
                    "arguments": arguments,
                    "service_name": service_name,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                    "message_count": len(result.messages)
                }
            else:
                # TODO: 权限控制 - 后续考虑添加提示词访问权限验证
                # TODO: 缓存策略 - 后续考虑添加提示词内容缓存

                # 尝试从所有服务获取提示词（找到第一个匹配的）
                services = self.registry.get_services(client_id)
                last_error = None

                for sname in services:
                    try:
                        client = self.registry.get_session(client_id, sname)
                        if client:
                            result = await client.get_prompt(name, arguments)
                            return {
                                "success": True,
                                "data": result.model_dump(),
                                "name": name,
                                "arguments": arguments,
                                "service_name": sname,
                                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                                "message_count": len(result.messages)
                            }
                    except Exception as e:
                        last_error = e
                        continue

                return {
                    "success": False,
                    "error": f"Prompt '{name}' not found in any service. Last error: {last_error}",
                    "data": None,
                    "name": name,
                    "arguments": arguments,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
                }

        except Exception as e:
            logger.error(f"Error getting prompt {name}: {e}")
            return {
                "success": False,
                "error": str(e),
                "data": None,
                "name": name,
                "arguments": arguments,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
            }
