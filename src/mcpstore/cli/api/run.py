"""
API 启动命令
"""
import typer
import os
import warnings
import uvicorn


def register_api_commands(app: typer.Typer) -> None:
    """注册 run/serve 命令"""

    @app.command("run")
    def run_command(
        service: str = typer.Argument(..., help="服务名称，目前仅支持 api"),
        host: str = typer.Option("0.0.0.0", "--host", "-h", help="监听地址"),
        port: int = typer.Option(18200, "--port", "-p", help="监听端口"),
        reload: bool = typer.Option(False, "--reload", "-r", help="开发模式自动重载"),
        log_level: str = typer.Option("info", "--log-level", "-l", help="日志级别"),
        prefix: str = typer.Option("", "--prefix", help="URL 前缀，例如 /api/v1"),
    ):
        """启动 MCPStore API 服务"""
        if service != "api":
            typer.echo("Only service=api is supported")
            raise typer.Exit(1)

        # Suppress known websockets legacy deprecation noise until uvicorn/websockets migrate to the new API.
        # Keeps logs clean without affecting functionality.
        os.environ.setdefault("PYTHONWARNINGS", "ignore::DeprecationWarning:websockets.legacy")
        warnings.filterwarnings("ignore", category=DeprecationWarning, module="websockets.legacy")
        # Suppress uvicorn importing deprecated WebSocketServerProtocol (websockets 14 legacy API).
        warnings.filterwarnings("ignore", category=DeprecationWarning, module="uvicorn.protocols.websockets.websockets_impl")

        try:
            typer.echo("[START] Starting MCPStore API Server")
            typer.echo(f"   Host: {host}:{port}")
            if prefix:
                typer.echo(f"   URL Prefix: {prefix}")
            typer.echo("   Press Ctrl+C to stop")

            if reload:
                if prefix:
                    typer.echo(" Reload mode does not support prefix, please remove --prefix or disable --reload")
                    raise typer.Exit(1)
                # 重载模式需使用字符串引用
                uvicorn.run(
                    "mcpstore.api.api_app:create_app",
                    host=host,
                    port=port,
                    reload=reload,
                    log_level=log_level,
                    factory=True,
                    reload_dirs=None,
                    reload_includes=None,
                    reload_excludes=None,
                )
            else:
                api_app = _create_api_app(prefix)
                uvicorn.run(
                    api_app,
                    host=host,
                    port=port,
                    reload=reload,
                    log_level=log_level,
                )
        except KeyboardInterrupt:
            typer.echo("\n[Stopped] User interrupted")
        except Exception as e:
            typer.echo(f" Startup failed: {e}")
            raise typer.Exit(1)

    @app.command("serve")
    def serve_alias(
        host: str = typer.Option("0.0.0.0", "--host", "-h", help="监听地址"),
        port: int = typer.Option(18200, "--port", "-p", help="监听端口"),
        reload: bool = typer.Option(False, "--reload", "-r", help="开发模式自动重载"),
        log_level: str = typer.Option("info", "--log-level", "-l", help="日志级别"),
        prefix: str = typer.Option("", "--prefix", help="URL 前缀，例如 /api/v1"),
    ):
        """serve 等价于 run api"""
        run_command(
            service="api",
            host=host,
            port=port,
            reload=reload,
            log_level=log_level,
            prefix=prefix,
        )


def _create_api_app(prefix: str):
    """创建 FastAPI 应用，应用 URL 前缀"""
    from mcpstore.api.api_app import create_app

    # 传入 prefix，避免旧路径中的固定空前缀
    return create_app(store=None, url_prefix=prefix or "")
