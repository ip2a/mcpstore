"""
Hub MCP Server Module
Hub MCP 服务器模块 - 将 MCPStore 对象暴露为 MCP 服务
"""

import logging
from typing import Union, Optional, Literal, Any, Callable, TYPE_CHECKING
import asyncio

from .types import HubMCPConfig, HubMCPStatus
from .exceptions import (
    ServerAlreadyRunningError,
    ServerNotRunningError,
    ToolExecutionError,
    PortBindingError,
)

if TYPE_CHECKING:
    from ..context.base_context import MCPStoreContext
    from ..context.service_proxy import ServiceProxy
    from mcpstore.core.models.tool import ToolInfo

logger = logging.getLogger(__name__)


class HubMCPServer:
    """
    Hub MCP 服务器
    
    将 MCPStore 对象暴露为标准 MCP 服务。
    基于 FastMCP 框架，提供薄包装层。
    
    核心理念：
    - 薄包装：直接使用 FastMCP 的能力
    - 工具转换：将 MCPStore 工具转换为 FastMCP 工具
    - 透传调用：工具调用直接转发到原始对象
    
    支持的对象类型：
    - Store 对象（MCPStoreContext with agent_id=None）
    - Agent 对象（MCPStoreContext with agent_id）
    - ServiceProxy 对象
    """
    
    def __init__(
        self,
        exposed_object: Union['MCPStoreContext', 'ServiceProxy'],
        transport: Literal["http", "sse", "stdio"] = "http",
        port: Optional[int] = None,
        host: str = "0.0.0.0",
        path: str = "/mcp",
        timeout: float = 30.0,
        **fastmcp_kwargs
    ):
        """
        初始化 Hub MCP 服务器
        
        Args:
            exposed_object: 要暴露的对象（Store/Agent/ServiceProxy）
            transport: 传输协议，可选 "http"、"sse"、"stdio"
            port: 端口号（仅 http/sse），None 为自动分配
            host: 监听地址（仅 http/sse），默认 "0.0.0.0"
            path: 端点路径（仅 http），默认 "/mcp"
            timeout: 超时时间（秒），默认 30.0
            **fastmcp_kwargs: 传递给 FastMCP 的其他参数（如 auth）
            
        Example:
            # 暴露 Store 对象
            store = MCPStore.setup_store()
            hub = store.for_store().hub_mcp(port=8000)
            
            # 暴露 Agent 对象
            agent = store.for_agent("my-agent")
            hub = agent.hub_mcp(transport="sse", port=8001)
            
            # 暴露 ServiceProxy 对象
            service = agent.find_service("weather")
            hub = service.hub_mcp(transport="stdio")
        """
        # 保存暴露对象
        self._exposed_object = exposed_object
        
        # 创建配置对象
        self._config = HubMCPConfig(
            transport=transport,
            port=port,
            host=host,
            path=path,
            timeout=timeout,
            fastmcp_kwargs=fastmcp_kwargs
        )
        
        # 初始化状态
        self._status = HubMCPStatus.INITIALIZING
        self._fastmcp: Optional[Any] = None  # FastMCP 实例
        self._server_task: Optional[asyncio.Task] = None  # 服务器任务
        
        logger.info(
            f"[HubMCPServer] 初始化中 - "
            f"对象类型={type(exposed_object).__name__}, "
            f"传输协议={transport}, "
            f"端口={port or '自动分配'}"
        )
        
        # 创建 FastMCP 服务器
        self._create_fastmcp_server()
        
        # 注册工具
        self._register_tools()
        
        # 初始化完成，设置为停止状态
        self._status = HubMCPStatus.STOPPED
        
        logger.info(
            f"[HubMCPServer] 初始化完成 - "
            f"服务器名称={self._generate_server_name()}, "
            f"状态={self._status.value}"
        )
    
    def _generate_server_name(self) -> str:
        """
        生成服务器名称
        
        根据暴露对象的类型生成合适的服务器名称：
        - Agent 对象 → "MCPStore-Agent-{agent_id}"
        - ServiceProxy 对象 → "MCPStore-Service-{service_name}"
        - Store 对象 → "MCPStore-Store"
        
        Returns:
            str: 生成的服务器名称
        """
        try:
            # 检查是否是 Agent 对象（有 _agent_id 属性且不为 None）
            if hasattr(self._exposed_object, '_agent_id') and self._exposed_object._agent_id:
                agent_id = self._exposed_object._agent_id
                server_name = f"MCPStore-Agent-{agent_id}"
                logger.debug(f"[HubMCPServer] 生成 Agent 服务器名称: {server_name}")
                return server_name
            
            # 检查是否是 ServiceProxy 对象（有 service_name 属性）
            if hasattr(self._exposed_object, 'service_name'):
                service_name = self._exposed_object.service_name
                server_name = f"MCPStore-Service-{service_name}"
                logger.debug(f"[HubMCPServer] 生成 ServiceProxy 服务器名称: {server_name}")
                return server_name
            
            # 默认为 Store 对象
            server_name = "MCPStore-Store"
            logger.debug(f"[HubMCPServer] 生成 Store 服务器名称: {server_name}")
            return server_name
            
        except Exception as e:
            logger.warning(f"[HubMCPServer] 生成服务器名称失败: {e}，使用默认名称")
            return "MCPStore-Hub"
    
    def _create_fastmcp_server(self) -> None:
        """
        创建 FastMCP 服务器实例
        
        使用生成的服务器名称和配置参数创建 FastMCP 实例。
        """
        try:
            # 导入 FastMCP
            from fastmcp import FastMCP
            
            # 生成服务器名称
            server_name = self._generate_server_name()
            
            # 创建 FastMCP 实例
            self._fastmcp = FastMCP(
                name=server_name,
                **self._config.fastmcp_kwargs
            )
            
            logger.info(f"[HubMCPServer] FastMCP 服务器创建成功: {server_name}")
            
        except ImportError as e:
            logger.error(f"[HubMCPServer] 无法导入 FastMCP: {e}")
            raise ImportError(
                "FastMCP 未安装。请运行: uv add fastmcp"
            ) from e
        except Exception as e:
            logger.error(f"[HubMCPServer] 创建 FastMCP 服务器失败: {e}")
            raise
    
    def _register_tools(self) -> None:
        """
        注册所有工具到 FastMCP
        
        从暴露对象获取工具列表，为每个工具创建代理函数，
        然后使用 FastMCP 的 @tool 装饰器注册。
        """
        try:
            # 获取工具列表
            tools = self._exposed_object.list_tools()
            
            logger.info(f"[HubMCPServer] 开始注册工具，共 {len(tools)} 个")
            
            # 为每个工具创建代理函数并注册
            registered_count = 0
            failed_count = 0
            
            for tool_info in tools:
                try:
                    # 创建代理工具
                    proxy_tool = self._create_proxy_tool(tool_info)
                    
                    # 使用 FastMCP 的 @tool 装饰器注册
                    self._fastmcp.tool(proxy_tool)
                    
                    registered_count += 1
                    logger.debug(f"[HubMCPServer] 工具注册成功: {tool_info.name}")
                    
                except Exception as e:
                    failed_count += 1
                    logger.warning(
                        f"[HubMCPServer] 工具注册失败: {tool_info.name}, "
                        f"错误: {e}"
                    )
                    # 单个工具注册失败不影响其他工具
                    continue
            
            logger.info(
                f"[HubMCPServer] 工具注册完成 - "
                f"成功: {registered_count}, 失败: {failed_count}"
            )
            
        except Exception as e:
            logger.error(f"[HubMCPServer] 注册工具失败: {e}")
            raise
    
    def _create_proxy_tool(self, tool_info: 'ToolInfo') -> Callable:
        """
        创建代理工具函数
        
        为指定的工具创建一个异步代理函数，该函数会将调用转发到
        原始对象的 call_tool_async 方法。
        
        Args:
            tool_info: 工具信息对象
            
        Returns:
            Callable: 代理函数，可以被 FastMCP 注册
        """
        # 提取工具元数据
        tool_name = tool_info.name
        tool_description = tool_info.description or f"工具: {tool_name}"
        
        # 创建异步代理函数
        async def proxy_tool(**kwargs) -> Any:
            """代理工具函数 - 将调用转发到原始对象"""
            try:
                logger.debug(
                    f"[HubMCPServer] 调用工具 '{tool_name}' - "
                    f"参数: {kwargs}"
                )
                
                # 调用原始对象的 call_tool_async 方法
                result = await self._exposed_object.call_tool_async(
                    tool_name=tool_name,
                    arguments=kwargs
                )
                
                logger.debug(
                    f"[HubMCPServer] 工具 '{tool_name}' 返回结果: {result}"
                )
                
                return result
                
            except Exception as e:
                logger.error(
                    f"[HubMCPServer] 工具 '{tool_name}' 执行失败: {e}"
                )
                raise ToolExecutionError(
                    f"工具 '{tool_name}' 执行失败: {str(e)}"
                ) from e
        
        # 设置函数元数据
        proxy_tool.__name__ = tool_name
        proxy_tool.__doc__ = tool_description
        
        # TODO: 动态设置参数注解（用于 FastMCP 的 schema 生成）
        # 这将在后续任务中实现（阶段 6）
        
        return proxy_tool
    
    @property
    def status(self) -> HubMCPStatus:
        """
        获取服务器状态
        
        Returns:
            HubMCPStatus: 当前服务器状态
        """
        return self._status
    
    @property
    def is_running(self) -> bool:
        """
        检查服务器是否运行中
        
        Returns:
            bool: 如果服务器正在运行返回 True，否则返回 False
        """
        return self._status == HubMCPStatus.RUNNING
    
    @property
    def endpoint_url(self) -> str:
        """
        获取服务器端点 URL
        
        根据传输协议返回不同格式的 URL：
        - stdio: "stdio://local"
        - sse: "http://{host}:{port}/sse"
        - http: "http://{host}:{port}{path}"
        
        Returns:
            str: 服务器端点 URL
        """
        if self._config.transport == "stdio":
            return "stdio://local"
        elif self._config.transport == "sse":
            return f"http://{self._config.host}:{self._config.port}/sse"
        else:  # http
            return f"http://{self._config.host}:{self._config.port}{self._config.path}"
    
    def __repr__(self) -> str:
        """字符串表示"""
        return (
            f"HubMCPServer("
            f"object={type(self._exposed_object).__name__}, "
            f"transport={self._config.transport}, "
            f"status={self._status.value}, "
            f"endpoint={self.endpoint_url}"
            f")"
        )
