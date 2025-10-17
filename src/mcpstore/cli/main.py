#!/usr/bin/env python3
"""
MCPStore CLI - Command Line Interface for MCPStore
"""
import sys
from typing import Optional

import typer
import uvicorn
from typing_extensions import Annotated

# Create main CLI application
app = typer.Typer(
    name="mcpstore",
    help="MCPStore - A composable, ready-to-use MCP toolkit for agents and rapid integration.",
    no_args_is_help=True,
    rich_markup_mode="rich"
)

@app.callback()
def callback():
    """
    MCPStore Command Line Interface

    A powerful toolkit for managing MCP (Model Context Protocol) services.
    """
    pass

@app.command("run")
def run_command(
    service: Annotated[str, typer.Argument(help="Service to run (api, test, etc.)")],
    host: Annotated[str, typer.Option("--host", "-h", help="Host to bind to")] = "0.0.0.0",
    port: Annotated[int, typer.Option("--port", "-p", help="Port to bind to")] = 18200,
    reload: Annotated[bool, typer.Option("--reload", "-r", help="Enable auto-reload")] = False,
    log_level: Annotated[str, typer.Option("--log-level", "-l", help="Log level")] = "info",
    prefix: Annotated[str, typer.Option("--prefix", help="URL prefix (e.g., /api/v1)")] = "",  # 🆕 新增
):
    """
    Run MCPStore services

    Available services:
    - api: Start the MCPStore API server

    Examples:

    # 基本启动（无前缀）
    mcpstore run api

    # 使用 URL 前缀
    mcpstore run api --prefix /api/v1
    # 访问: http://localhost:18200/api/v1/for_store/list_services

    # 开发模式 + 前缀
    mcpstore run api --reload --prefix /api
    """
    if service == "api":
        run_api(host=host, port=port, reload=reload, log_level=log_level, url_prefix=prefix)
    else:
        typer.echo(f" Unknown service: {service}")
        typer.echo("Available services: api")
        raise typer.Exit(1)

def run_api(host: str, port: int, reload: bool, log_level: str, url_prefix: str):
    """Start MCPStore API service"""
    try:
        typer.echo("🚀 Starting MCPStore API Server...")
        typer.echo(f"   Host: {host}:{port}")

        if url_prefix:
            typer.echo(f"   URL Prefix: {url_prefix}")
            base_url = f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}"
            typer.echo(f"   Example: {base_url}{url_prefix}/for_store/list_services")
        else:
            base_url = f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}"
            typer.echo(f"   Example: {base_url}/for_store/list_services")

        if reload:
            typer.echo("   Mode: Development (auto-reload enabled)")
        typer.echo("   Press Ctrl+C to stop")
        typer.echo()

        # 🆕 启动时传入 URL 前缀配置
        # 通过环境变量传递给应用工厂
        import os
        if url_prefix:
            os.environ["MCPSTORE_URL_PREFIX"] = url_prefix

        # Start API service
        uvicorn.run(
            "mcpstore.scripts.app:app",
            host=host,
            port=port,
            reload=reload,
            log_level=log_level
        )
    except KeyboardInterrupt:
        typer.echo("\n🛑 Server stopped by user")
    except Exception as e:
        typer.echo(f" Failed to start server: {e}")
        raise typer.Exit(1)

@app.command("version")
def version():
    """Show version information"""
    try:
        from mcpstore import __version__
        version_str = __version__
    except ImportError:
        version_str = "0.2.0"

    typer.echo(f"MCPStore version: {version_str}")

@app.command("test")
def test_command(
    suite: Annotated[
        Optional[str],
        typer.Argument(help="Test suite to run")
    ] = "all",
    host: Annotated[str, typer.Option("--host", help="API server host")] = "localhost",
    port: Annotated[int, typer.Option("--port", help="API server port")] = 18611,
    verbose: Annotated[bool, typer.Option("--verbose", "-v", help="Verbose output")] = False,
    performance: Annotated[bool, typer.Option("--performance", "-p", help="Include performance tests")] = False,
    max_concurrent: Annotated[int, typer.Option("--max-concurrent", help="Max concurrent requests for performance tests")] = 10,
):
    """
    Run MCPStore tests

    Available test suites:
    - health: Quick health check
    - smoke: Smoke tests (basic functionality)
    - api: Basic API tests
    - core: Core functionality tests
    - advanced: Advanced API tests
    - performance: Performance and load tests
    - comprehensive: All tests including performance
    - all: Basic tests (default)
    """
    try:
        import asyncio
        from mcpstore.cli.test_runner import run_tests

        # For comprehensive testing, use special handling
        if suite == "comprehensive":
            from mcpstore.cli.comprehensive_test import run_comprehensive_tests
            base_url = f"http://{host}:{port}"
            success = asyncio.run(run_comprehensive_tests(
                base_url=base_url,
                include_performance=performance,
                max_concurrent=max_concurrent,
                verbose=verbose
            ))
        else:
            success = asyncio.run(run_tests(suite=suite, host=host, port=port, verbose=verbose))

        if not success:
            raise typer.Exit(1)
    except ImportError as e:
        typer.echo(f" Test runner not available: {e}")
        raise typer.Exit(1)
    except Exception as e:
        typer.echo(f" Test failed: {e}")
        raise typer.Exit(1)

@app.command("config")
def config_command(
    action: Annotated[str, typer.Argument(help="Action: show, validate, init")],
    path: Annotated[Optional[str], typer.Option("--path", help="Config file path")] = None,
):
    """
    Manage MCPStore configuration

    Actions:
    - show: Display current configuration
    - validate: Validate configuration file
    - init: Initialize default configuration
    """
    try:
        from mcpstore.cli.config_manager import handle_config
        handle_config(action=action, path=path)
    except ImportError:
        typer.echo(" Config manager not available")
        raise typer.Exit(1)
    except Exception as e:
        typer.echo(f" Config operation failed: {e}")
        raise typer.Exit(1)

def main():
    """CLI entry point"""
    try:
        app()
    except KeyboardInterrupt:
        typer.echo("\n👋 Goodbye!")
        sys.exit(0)
    except Exception as e:
        typer.echo(f" CLI error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
