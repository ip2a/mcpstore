"""
Hub MCP Server Module
Hub MCP 服务器模块 - 将 MCPStore 对象暴露为 MCP 服务
"""

import asyncio
import keyword
import logging
import threading
from contextlib import suppress
from typing import Union, Optional, Literal, Any, Callable, TYPE_CHECKING

from .exceptions import (
    ServerAlreadyRunningError,
    ServerNotRunningError,
    PortBindingError,
)
from .types import HubMCPConfig, HubMCPStatus

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
            fastmcp_kwargs=fastmcp_kwargs
        )
        
        # 初始化状态
        self._status = HubMCPStatus.STARTUP
        self._fastmcp: Optional[Any] = None  # FastMCP 实例
        self._server_task: Optional[asyncio.Task] = None  # 服务器任务
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._background_thread: Optional[threading.Thread] = None
        
        logger.info(
            f"[HubMCPServer] Initializing - "
            f"object_type={type(exposed_object).__name__}, "
            f"transport={transport}, "
            f"port={port or 'auto-assign'}"
        )
        
        # 创建 FastMCP 服务器
        self._create_fastmcp_server()

        # 注册工具
        self._register_tools()

        # 初始化完成，设置为停止状态
        self._status = HubMCPStatus.STOPPED
        
        logger.info(
            f"[HubMCPServer] Initialization completed - "
            f"server_name={self._generate_server_name()}, "
            f"status={self._status.value}"
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
                logger.debug(f"[HubMCPServer] [NAME] Generated Agent server name: {server_name}")
                return server_name
            
            # 检查是否是 ServiceProxy 对象（有 service_name 属性）
            if hasattr(self._exposed_object, 'service_name'):
                service_name = self._exposed_object.service_name
                server_name = f"MCPStore-Service-{service_name}"
                logger.debug(f"[HubMCPServer] [NAME] Generated ServiceProxy server name: {server_name}")
                return server_name
            
            # 默认为 Store 对象
            server_name = "MCPStore-Store"
            logger.debug(f"[HubMCPServer] [NAME] Generated Store server name: {server_name}")
            return server_name
            
        except Exception as e:
            logger.warning(f"[HubMCPServer] [WARN] Failed to generate server name: {e}, using default name")
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
            
            logger.info(f"[HubMCPServer] [SUCCESS] FastMCP server created successfully: {server_name}")
            
        except ImportError as e:
            logger.error(f"[HubMCPServer] [ERROR] Unable to import FastMCP: {e}")
            raise ImportError(
                "FastMCP is not installed. Please run: uv add fastmcp"
            ) from e
        except Exception as e:
            logger.error(f"[HubMCPServer] [ERROR] Failed to create FastMCP server: {e}")
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
            
            logger.info(f"[HubMCPServer] [REGISTER] Starting to register tools, total {len(tools)}")
            
            # 为每个工具创建代理函数并注册
            registered_count = 0
            failed_count = 0
            
            for tool_info in tools:
                try:
                    # 创建代理工具
                    proxy_tool = self._create_proxy_tool(tool_info)

                    annotations = None
                    schema = getattr(tool_info, "inputSchema", None)
                    if schema and isinstance(schema, dict):
                        annotations = {"arguments": schema}

                    meta = {
                        "service_name": getattr(tool_info, "service_name", None),
                        "service_global_name": getattr(tool_info, "service_global_name", None),
                        "client_id": getattr(tool_info, "client_id", None),
                    }

                    description = tool_info.description or f"工具: {tool_info.name}"
                    decorator_kwargs = {
                        "name": tool_info.name,
                        "description": description,
                        "meta": {k: v for k, v in meta.items() if v is not None},
                    }
                    if annotations:
                        decorator_kwargs["annotations"] = annotations

                    decorator = self._fastmcp.tool(**decorator_kwargs)
                    decorator(proxy_tool)

                    registered_count += 1
                    logger.debug(f"[HubMCPServer] [SUCCESS] Tool registered successfully: {tool_info.name}")

                except Exception as e:
                    failed_count += 1
                    logger.warning(
                        f"[HubMCPServer] [WARN] Tool registration failed: {tool_info.name}, "
                        f"error: {e}"
                    )
                    # 单个工具注册失败不影响其他工具
                    continue
            
            logger.info(
                f"[HubMCPServer] [COMPLETE] Tool registration completed - "
                f"successful: {registered_count}, failed: {failed_count}"
            )
            
        except Exception as e:
            logger.error(f"[HubMCPServer] [ERROR] Tool registration failed: {e}")
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
        schema = getattr(tool_info, "inputSchema", {}) or {}
        properties = schema.get("properties") or {}
        required = set(schema.get("required") or [])

        params_code: list[str] = []
        arg_lines: list[str] = []

        for original_name, prop_schema in properties.items():
            safe_name = self._sanitize_param_name(original_name)

            if original_name in required:
                params_code.append(f"{safe_name}")
            else:
                default_value = prop_schema.get("default", None)
                params_code.append(f"{safe_name}={repr(default_value)}")

            arg_lines.append(f"    arguments['{original_name}'] = {safe_name}")

        params_signature = ", ".join(params_code)
        body_lines = ["    arguments = {}"]
        body_lines.extend(arg_lines)
        body_lines.append("    return await __call_tool(tool_name=__tool_name, args=arguments)")

        function_code = "async def handler({signature}):\n{body}\n".format(
            signature=params_signature,
            body="\n".join(body_lines) if body_lines else "    return await __call_tool(tool_name=__tool_name, args={})",
        )

        namespace = {
            "__call_tool": self._exposed_object.call_tool_async,
            "__tool_name": tool_info.name,
        }

        try:
            exec(function_code, namespace)
        except Exception as exc:  # noqa: BLE001
            logger.error(f"[HubMCPServer] Failed to generate proxy function for '{tool_info.name}': {exc}")
            raise

        proxy_tool = namespace["handler"]
        proxy_tool.__name__ = tool_info.name
        proxy_tool.__doc__ = (tool_info.description or f"工具: {tool_info.name}")

        return proxy_tool

    def _sanitize_param_name(self, name: str) -> str:
        if not isinstance(name, str):
            raise ValueError(f"Illegal parameter name: {name}")
        if not name.isidentifier() or keyword.iskeyword(name):
            raise ValueError(f"Tool parameter name '{name}' is not a valid Python identifier")
        return name
    
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

    # ---- Lifecycle helpers -------------------------------------------------

    def _get_transport_kwargs(self) -> dict[str, Any]:
        """根据传输协议构造 FastMCP 运行参数。"""
        transport = self._config.transport
        if transport in {"http", "sse", "streamable-http"}:
            kwargs: dict[str, Any] = {}
            if self._config.host:
                kwargs["host"] = self._config.host
            if self._config.port is not None:
                kwargs["port"] = self._config.port
            if transport in {"http", "sse"} and self._config.path:
                kwargs["path"] = self._config.path
            return kwargs
        return {}

    async def _run_server(self, show_banner: bool) -> None:
        """启动 FastMCP 服务器的核心协程。"""
        if self.is_running:
            raise ServerAlreadyRunningError("Hub MCP server is already running")

        self._status = HubMCPStatus.RUNNING
        try:
            await self._fastmcp.run_async(
                transport=self._config.transport,
                show_banner=show_banner,
                **self._get_transport_kwargs(),
            )
        except asyncio.CancelledError:
            logger.info("[HubMCPServer] [STOP] Received stop signal, shutting down")
            self._status = HubMCPStatus.STOPPED
            raise
        except OSError as exc:
            self._status = HubMCPStatus.ERROR
            raise PortBindingError(
                f"无法绑定端口 {self._config.port}: {exc}"
            ) from exc
        except Exception as exc:  # noqa: BLE001
            self._status = HubMCPStatus.ERROR
            logger.error(f"[HubMCPServer] [ERROR] Server run failed: {exc}")
            raise
        else:
            self._status = HubMCPStatus.STOPPED

    async def start_async(self, show_banner: bool = False) -> asyncio.Task:
        """在当前事件循环中以后台任务形式启动服务器。"""
        loop = asyncio.get_running_loop()
        if self._server_task and not self._server_task.done():
            raise ServerAlreadyRunningError("Hub MCP server is already running")

        self._server_task = loop.create_task(
            self._run_server(show_banner=show_banner),
            name=f"HubMCPServer({self._generate_server_name()})",
        )
        return self._server_task

    def start(self, *, block: bool = False, show_banner: bool = False) -> "HubMCPServer":
        """
        启动服务器。

        block=False 时在后台事件循环运行；block=True 会阻塞当前线程直到服务器退出。
        """
        self._start_in_background(show_banner=show_banner)
        if block:
            self.wait()
        return self

    def _start_in_background(self, show_banner: bool) -> None:
        if self._background_thread and self._background_thread.is_alive():
            raise ServerAlreadyRunningError("Hub MCP server is already running in background")

        loop = asyncio.new_event_loop()
        self._loop = loop

        def _target() -> None:
            asyncio.set_event_loop(loop)
            self._server_task = loop.create_task(self._run_server(show_banner=show_banner))
            try:
                loop.run_until_complete(self._server_task)
            except asyncio.CancelledError:
                logger.debug("[HubMCPServer] [CANCEL] Background task cancelled")
            finally:
                self._server_task = None
                self._loop = None
                self._background_thread = None
                loop.close()

        thread = threading.Thread(
            target=_target,
            name=f"HubMCPServer-{self._generate_server_name()}",
            daemon=True,
        )
        self._background_thread = thread
        thread.start()

    async def stop_async(self) -> None:
        """在当前事件循环中停止服务器。"""
        if not self._server_task:
            raise ServerNotRunningError("Hub MCP server is not running")

        self._status = HubMCPStatus.STOPPING
        self._server_task.cancel()
        with suppress(asyncio.CancelledError):
            await self._server_task
        self._server_task = None
        self._status = HubMCPStatus.STOPPED

    def stop(self, timeout: float | None = None) -> None:
        """停止后台运行的服务器。"""
        if self._loop and self._server_task:
            self._status = HubMCPStatus.STOPPING
            future = asyncio.run_coroutine_threadsafe(self._cancel_task(), self._loop)
            future.result(timeout)
            if self._background_thread:
                self._background_thread.join(timeout)
            self._status = HubMCPStatus.STOPPED
            return

        if self._server_task and not self._server_task.done():
            raise RuntimeError("Hub MCP is running in the current event loop, please use stop_async()")

        raise ServerNotRunningError("Hub MCP server is not running")

    async def _cancel_task(self) -> None:
        if self._server_task:
            self._server_task.cancel()
            with suppress(asyncio.CancelledError):
                await self._server_task
            self._server_task = None

    def restart(self, *, block: bool = False, show_banner: bool = False) -> "HubMCPServer":
        """重新启动服务器。"""
        if self.is_running:
            self.stop()
        return self.start(block=block, show_banner=show_banner)

    def wait(self, timeout: float | None = None) -> None:
        """阻塞当前线程直到后台服务器退出。"""
        if not self._background_thread:
            raise ServerNotRunningError("Hub MCP server is not running in background")
        self._background_thread.join(timeout)
