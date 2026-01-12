"""
配置相关命令封装
"""
from typing import Optional

import typer

from mcpstore.cli.config_manager import handle_config


def register_config_commands(app: typer.Typer) -> None:
    """注册 config 子命令"""

    @app.command("config")
    def config_command(
        action: str = typer.Argument(..., help="操作：show/validate/init/add-examples/path"),
        path: Optional[str] = typer.Option(None, "--path", help="配置文件路径"),
        force: bool = typer.Option(False, "--force", help="覆盖已存在文件（init 可用）"),
        with_examples: bool = typer.Option(False, "--with-examples", help="初始化时包含示例服务"),
    ):
        """管理 MCPStore 配置文件"""
        try:
            handle_config(action=action, path=path, force=force, with_examples=with_examples)
        except Exception as e:
            typer.echo(f"[错误] 配置操作失败: {e}")
            raise typer.Exit(1)
