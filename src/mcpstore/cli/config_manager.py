#!/usr/bin/env python3
"""
MCPStore Configuration Manager - 配置文件管理工具
"""
import json
import os
import typer
from pathlib import Path
from typing import Dict, Any, Optional

def get_default_config_path() -> Path:
    """获取默认配置文件路径"""
    # 优先级：当前目录 > 用户目录 > 系统目录
    paths = [
        Path.cwd() / "mcp.json",
        Path.home() / ".mcpstore" / "mcp.json",
        Path("/etc/mcpstore/mcp.json") if os.name != 'nt' else Path(os.environ.get('PROGRAMDATA', 'C:\\ProgramData')) / "mcpstore" / "mcp.json"
    ]
    
    for path in paths:
        if path.exists():
            return path
    
    # 如果都不存在，返回当前目录
    return paths[0]

def get_default_config() -> Dict[str, Any]:
    """获取默认配置"""
    return {
        "mcpServers": {
            "example-service": {
                "command": "python",
                "args": ["-m", "example_mcp_server"],
                "env": {},
                "description": "Example MCP service"
            }
        },
        "version": "0.2.0",
        "description": "MCPStore default configuration"
    }

def load_config(path: Optional[str] = None) -> Dict[str, Any]:
    """加载配置文件"""
    if path:
        config_path = Path(path)
    else:
        config_path = get_default_config_path()
    
    if not config_path.exists():
        typer.echo(f"⚠️  Configuration file not found: {config_path}")
        return {}
    
    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        typer.echo(f"✅ Configuration loaded from: {config_path}")
        return config
    except json.JSONDecodeError as e:
        typer.echo(f"❌ Invalid JSON in config file: {e}")
        return {}
    except Exception as e:
        typer.echo(f"❌ Failed to load config: {e}")
        return {}

def save_config(config: Dict[str, Any], path: Optional[str] = None) -> bool:
    """保存配置文件"""
    if path:
        config_path = Path(path)
    else:
        config_path = get_default_config_path()
    
    try:
        # 确保目录存在
        config_path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f, indent=2, ensure_ascii=False)
        
        typer.echo(f"✅ Configuration saved to: {config_path}")
        return True
    except Exception as e:
        typer.echo(f"❌ Failed to save config: {e}")
        return False

def validate_config(config: Dict[str, Any]) -> bool:
    """验证配置文件格式"""
    errors = []
    
    # 检查必需字段
    if "mcpServers" not in config:
        errors.append("Missing 'mcpServers' field")
    else:
        servers = config["mcpServers"]
        if not isinstance(servers, dict):
            errors.append("'mcpServers' must be an object")
        else:
            for name, server_config in servers.items():
                if not isinstance(server_config, dict):
                    errors.append(f"Server '{name}' config must be an object")
                    continue
                
                # 检查服务配置
                if "command" not in server_config:
                    errors.append(f"Server '{name}' missing 'command' field")
                
                if "args" in server_config and not isinstance(server_config["args"], list):
                    errors.append(f"Server '{name}' 'args' must be a list")
                
                if "env" in server_config and not isinstance(server_config["env"], dict):
                    errors.append(f"Server '{name}' 'env' must be an object")
    
    if errors:
        typer.echo("❌ Configuration validation failed:")
        for error in errors:
            typer.echo(f"   • {error}")
        return False
    else:
        typer.echo("✅ Configuration is valid")
        return True

def show_config(path: Optional[str] = None):
    """显示配置文件内容"""
    config = load_config(path)
    
    if not config:
        typer.echo("No configuration found")
        return
    
    typer.echo("\n📋 Current Configuration:")
    typer.echo("─" * 50)
    
    # 显示基本信息
    version = config.get("version", "unknown")
    description = config.get("description", "No description")
    typer.echo(f"Version: {version}")
    typer.echo(f"Description: {description}")
    
    # 显示服务列表
    servers = config.get("mcpServers", {})
    typer.echo(f"\n🔧 MCP Services ({len(servers)} configured):")
    
    if not servers:
        typer.echo("   No services configured")
    else:
        for name, server_config in servers.items():
            command = server_config.get("command", "unknown")
            args = server_config.get("args", [])
            desc = server_config.get("description", "No description")
            
            typer.echo(f"\n   📦 {name}")
            typer.echo(f"      Command: {command}")
            if args:
                typer.echo(f"      Args: {' '.join(args)}")
            typer.echo(f"      Description: {desc}")
            
            # 显示环境变量
            env = server_config.get("env", {})
            if env:
                typer.echo(f"      Environment:")
                for key, value in env.items():
                    typer.echo(f"        {key}={value}")

def init_config(path: Optional[str] = None, force: bool = False):
    """初始化默认配置文件"""
    if path:
        config_path = Path(path)
    else:
        config_path = get_default_config_path()
    
    if config_path.exists() and not force:
        typer.echo(f"⚠️  Configuration file already exists: {config_path}")
        typer.echo("Use --force to overwrite")
        return
    
    default_config = get_default_config()
    
    if save_config(default_config, str(config_path)):
        typer.echo("🎉 Default configuration initialized!")
        typer.echo(f"📁 Location: {config_path}")
        typer.echo("\n💡 You can now edit the configuration file to add your MCP services.")

def handle_config(action: str, path: Optional[str] = None):
    """处理配置命令"""
    if action == "show":
        show_config(path)
    elif action == "validate":
        config = load_config(path)
        if config:
            validate_config(config)
        else:
            typer.echo("❌ No configuration to validate")
    elif action == "init":
        force = typer.confirm("Overwrite existing configuration?") if path and Path(path).exists() else False
        init_config(path, force)
    else:
        typer.echo(f"❌ Unknown action: {action}")
        typer.echo("Available actions: show, validate, init")

if __name__ == "__main__":
    # 简单的命令行接口用于测试
    import sys
    
    if len(sys.argv) < 2:
        typer.echo("Usage: python config_manager.py <action> [path]")
        typer.echo("Actions: show, validate, init")
        sys.exit(1)
    
    action = sys.argv[1]
    path = sys.argv[2] if len(sys.argv) > 2 else None
    
    handle_config(action, path)
