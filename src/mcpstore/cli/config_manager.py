#!/usr/bin/env python3
"""
MCPStore Configuration Manager - Configuration file management tool
"""
import json
import os
import platform
from pathlib import Path
from typing import Dict, Any, Optional, List

import typer

# Configuration constants
class ConfigConstants:
    """Configuration related constants"""
    DEFAULT_VERSION = "1.0.0"
    CONFIG_FILENAME = "mcp.json"
    APP_NAME = "mcpstore"

    # UI constants
    SEPARATOR_LENGTH = 50
    SEPARATOR_CHAR = "─"

    # Supported service types
    SUPPORTED_TRANSPORTS = ["streamable-http", "sse", "stdio"]

    # Required field mapping
    REQUIRED_FIELDS = {
        "url": ["url"],  # Required fields for URL services
        "command": ["command"],  # Required fields for command services
    }


def _get_system_config_dir() -> Path:
    """Get system configuration directory (cross-platform)"""
    system = platform.system().lower()

    if system == "windows":
        # Windows: %PROGRAMDATA%\mcpstore
        program_data = os.environ.get('PROGRAMDATA', 'C:\\ProgramData')
        return Path(program_data) / ConfigConstants.APP_NAME
    elif system == "darwin":
        # macOS: /Library/Application Support/mcpstore
        return Path("/Library/Application Support") / ConfigConstants.APP_NAME
    else:
        # Linux/Unix: /etc/mcpstore
        return Path("/etc") / ConfigConstants.APP_NAME

def get_default_config_path() -> Path:
    """Get default configuration file path (search by priority)"""
    search_paths = [
        # 1. Current working directory
        Path.cwd() / ConfigConstants.CONFIG_FILENAME,
        # 2. User configuration directory
        Path.home() / f".{ConfigConstants.APP_NAME}" / ConfigConstants.CONFIG_FILENAME,
        # 3. System configuration directory
        _get_system_config_dir() / ConfigConstants.CONFIG_FILENAME
    ]

    # Return first existing file, if none exist return current directory
    for path in search_paths:
        if path.exists():
            return path

    return search_paths[0]

def get_default_config() -> Dict[str, Any]:
    """Get default configuration (empty configuration, avoid hardcoded examples)"""
    return {
        "mcpServers": {},
        "version": ConfigConstants.DEFAULT_VERSION,
        "description": "MCPStore configuration file",
        "created_by": "MCPStore CLI",
        "created_at": None  # Will be set when saving
    }

def get_example_services() -> Dict[str, Dict[str, Any]]:
    """Get example service configurations (for documentation and help)"""
    return {
        "remote-http-service": {
            "url": "https://example.com/mcp",
            "transport": "streamable-http",
            "headers": {},
            "description": "Example remote HTTP MCP service"
        },
        "local-command-service": {
            "command": "python",
            "args": ["-m", "your_mcp_server"],
            "env": {},
            "working_dir": ".",
            "description": "Example local command MCP service"
        },
        "npm-package-service": {
            "command": "npx",
            "args": ["-y", "some-mcp-package"],
            "description": "Example NPM package MCP service"
        }
    }

def load_config(path: Optional[str] = None) -> Dict[str, Any]:
    """Load configuration file"""
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
    """Save configuration file"""
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

def _detect_service_type(server_config: Dict[str, Any]) -> str:
    """检测服务类型"""
    if "url" in server_config:
        return "url"
    elif "command" in server_config:
        return "command"
    else:
        return "unknown"

def _validate_service_config(name: str, server_config: Dict[str, Any]) -> List[str]:
    """验证单个服务配置"""
    errors = []

    if not isinstance(server_config, dict):
        errors.append(f"Service '{name}' config must be an object")
        return errors

    service_type = _detect_service_type(server_config)

    if service_type == "unknown":
        errors.append(f"Service '{name}' must have either 'url' or 'command' field")
        return errors

    # 验证必需字段
    required_fields = ConfigConstants.REQUIRED_FIELDS.get(service_type, [])
    for field in required_fields:
        if field not in server_config:
            errors.append(f"Service '{name}' missing required field '{field}' for {service_type} type")

    # 验证字段类型
    type_validations = {
        "args": (list, "must be a list"),
        "env": (dict, "must be an object"),
        "headers": (dict, "must be an object"),
        "transport": (str, "must be a string"),
        "url": (str, "must be a string"),
        "command": (str, "must be a string"),
        "working_dir": (str, "must be a string"),
    }

    for field, (expected_type, error_msg) in type_validations.items():
        if field in server_config and not isinstance(server_config[field], expected_type):
            errors.append(f"Service '{name}' field '{field}' {error_msg}")

    # 验证transport值
    if "transport" in server_config:
        transport = server_config["transport"]
        if transport not in ConfigConstants.SUPPORTED_TRANSPORTS:
            errors.append(f"Service '{name}' unsupported transport '{transport}'. Supported: {', '.join(ConfigConstants.SUPPORTED_TRANSPORTS)}")

    return errors

def validate_config(config: Dict[str, Any]) -> bool:
    """验证配置文件格式"""
    errors = []

    # 检查根级必需字段
    if "mcpServers" not in config:
        errors.append("Missing 'mcpServers' field")
        typer.echo("❌ Configuration validation failed:")
        for error in errors:
            typer.echo(f"   • {error}")
        return False

    servers = config["mcpServers"]
    if not isinstance(servers, dict):
        errors.append("'mcpServers' must be an object")
    else:
        # 验证每个服务配置
        for name, server_config in servers.items():
            service_errors = _validate_service_config(name, server_config)
            errors.extend(service_errors)

    # 输出结果
    if errors:
        typer.echo("❌ Configuration validation failed:")
        for error in errors:
            typer.echo(f"   • {error}")
        return False
    else:
        typer.echo("✅ Configuration is valid")
        return True

def _format_service_info(name: str, server_config: Dict[str, Any]) -> None:
    """格式化并显示单个服务信息"""
    service_type = _detect_service_type(server_config)
    desc = server_config.get("description", "No description")

    # 服务类型图标
    type_icons = {
        "url": "🌐",
        "command": "📦",
        "unknown": "❓"
    }

    icon = type_icons.get(service_type, "❓")
    typer.echo(f"\n   {icon} {name} ({service_type} service)")
    typer.echo(f"      Description: {desc}")

    # 根据服务类型显示不同信息
    if service_type == "url":
        url = server_config.get("url", "")
        transport = server_config.get("transport", "streamable-http")
        typer.echo(f"      URL: {url}")
        typer.echo(f"      Transport: {transport}")

        headers = server_config.get("headers", {})
        if headers:
            typer.echo(f"      Headers:")
            for key, value in headers.items():
                typer.echo(f"        {key}: {value}")

    elif service_type == "command":
        command = server_config.get("command", "")
        args = server_config.get("args", [])
        working_dir = server_config.get("working_dir", "")

        typer.echo(f"      Command: {command}")
        if args:
            typer.echo(f"      Args: {' '.join(args)}")
        if working_dir:
            typer.echo(f"      Working Dir: {working_dir}")

        # 显示环境变量
        env = server_config.get("env", {})
        if env:
            typer.echo(f"      Environment:")
            for key, value in env.items():
                typer.echo(f"        {key}={value}")

def show_config(path: Optional[str] = None):
    """显示配置文件内容"""
    config = load_config(path)

    if not config:
        typer.echo("No configuration found")
        return

    separator = ConfigConstants.SEPARATOR_CHAR * ConfigConstants.SEPARATOR_LENGTH

    typer.echo("\n📋 Current Configuration:")
    typer.echo(separator)

    # 显示基本信息
    version = config.get("version", "unknown")
    description = config.get("description", "No description")
    created_by = config.get("created_by", "Unknown")

    typer.echo(f"Version: {version}")
    typer.echo(f"Description: {description}")
    typer.echo(f"Created by: {created_by}")

    # 显示服务列表
    servers = config.get("mcpServers", {})
    typer.echo(f"\n🔧 MCP Services ({len(servers)} configured):")

    if not servers:
        typer.echo("   No services configured")
        typer.echo("\n💡 Tip: Use 'mcpstore config add-example' to add example services")
    else:
        for name, server_config in servers.items():
            _format_service_info(name, server_config)

def init_config(path: Optional[str] = None, force: bool = False, with_examples: bool = False):
    """初始化配置文件"""
    if path:
        config_path = Path(path)
    else:
        config_path = get_default_config_path()

    if config_path.exists() and not force:
        typer.echo(f"⚠️  Configuration file already exists: {config_path}")
        typer.echo("Use --force to overwrite")
        return

    # 获取基础配置
    config = get_default_config()

    # 添加创建时间
    from datetime import datetime
    config["created_at"] = datetime.now().isoformat()

    # 如果需要示例，添加示例服务
    if with_examples:
        config["mcpServers"] = get_example_services()
        typer.echo("📝 Including example services in configuration")

    if save_config(config, str(config_path)):
        typer.echo("🎉 Configuration initialized successfully!")
        typer.echo(f"📁 Location: {config_path}")

        if with_examples:
            typer.echo("\n💡 Example services have been added. Edit the file to customize them.")
        else:
            typer.echo("\n💡 Empty configuration created. Add services using 'mcpstore config add' or edit the file manually.")

def add_example_services(path: Optional[str] = None):
    """向现有配置添加示例服务"""
    config = load_config(path)
    if not config:
        typer.echo("❌ No configuration found. Use 'init' first.")
        return

    examples = get_example_services()
    servers = config.get("mcpServers", {})

    added_count = 0
    for name, service_config in examples.items():
        if name not in servers:
            servers[name] = service_config
            added_count += 1
            typer.echo(f"✅ Added example service: {name}")
        else:
            typer.echo(f"⚠️  Service '{name}' already exists, skipping")

    if added_count > 0:
        config["mcpServers"] = servers
        if save_config(config, path):
            typer.echo(f"\n🎉 Added {added_count} example services!")
    else:
        typer.echo("\n💡 No new services were added.")

def handle_config(action: str, path: Optional[str] = None, **kwargs):
    """处理配置命令（改进版）"""
    actions = {
        "show": lambda: show_config(path),
        "validate": lambda: _handle_validate(path),
        "init": lambda: _handle_init(path, **kwargs),
        "add-examples": lambda: add_example_services(path),
        "path": lambda: _show_config_path(path),
    }

    if action in actions:
        actions[action]()
    else:
        typer.echo(f"❌ Unknown action: {action}")
        typer.echo(f"Available actions: {', '.join(actions.keys())}")

def _handle_validate(path: Optional[str] = None):
    """处理验证命令"""
    config = load_config(path)
    if config:
        validate_config(config)
    else:
        typer.echo("❌ No configuration to validate")

def _handle_init(path: Optional[str] = None, **kwargs):
    """处理初始化命令"""
    force = kwargs.get('force', False)
    with_examples = kwargs.get('with_examples', False)

    # 如果文件存在且没有force标志，询问用户
    config_path = Path(path) if path else get_default_config_path()
    if config_path.exists() and not force:
        force = typer.confirm("Configuration file exists. Overwrite?")

    init_config(path, force, with_examples)

def _show_config_path(path: Optional[str] = None):
    """显示配置文件路径"""
    if path:
        config_path = Path(path)
    else:
        config_path = get_default_config_path()

    typer.echo(f"📁 Configuration file path: {config_path}")
    typer.echo(f"📊 Exists: {'Yes' if config_path.exists() else 'No'}")

    if config_path.exists():
        stat = config_path.stat()
        typer.echo(f"📏 Size: {stat.st_size} bytes")
        from datetime import datetime
        modified_time = datetime.fromtimestamp(stat.st_mtime)
        typer.echo(f"🕒 Last modified: {modified_time.strftime('%Y-%m-%d %H:%M:%S')}")

# 改进的命令行接口
if __name__ == "__main__":
    app = typer.Typer(help="MCPStore Configuration Manager")

    @app.command()
    def show(path: Optional[str] = typer.Option(None, help="Configuration file path")):
        """Show current configuration"""
        show_config(path)

    @app.command()
    def validate(path: Optional[str] = typer.Option(None, help="Configuration file path")):
        """Validate configuration file"""
        _handle_validate(path)

    @app.command()
    def init(
        path: Optional[str] = typer.Option(None, help="Configuration file path"),
        force: bool = typer.Option(False, "--force", help="Overwrite existing file"),
        with_examples: bool = typer.Option(False, "--examples", help="Include example services")
    ):
        """Initialize configuration file"""
        _handle_init(path, force=force, with_examples=with_examples)

    @app.command("add-examples")
    def add_examples(path: Optional[str] = typer.Option(None, help="Configuration file path")):
        """Add example services to existing configuration"""
        add_example_services(path)

    @app.command()
    def path(path: Optional[str] = typer.Option(None, help="Configuration file path")):
        """Show configuration file path and info"""
        _show_config_path(path)

    app()
