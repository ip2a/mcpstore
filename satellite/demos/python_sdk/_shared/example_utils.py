"""Shared helpers for the installed-package Python SDK demos."""

from __future__ import annotations

import os
from pathlib import Path


DEMO_DIR = Path(__file__).resolve().parent.parent


def _load_env(path: Path = DEMO_DIR / ".env") -> None:
    """Load a small dotenv file without adding a runtime dependency."""
    if not path.is_file():
        return
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key:
            os.environ.setdefault(key, value)


def _optional_int(name: str, default: int) -> int:
    value = os.getenv(name)
    return int(value) if value else default


_load_env()


class ExampleConfig:
    """Optional demo settings loaded from ``demos/python_sdk/.env``."""

    REDIS_HOST = os.getenv("MCPSTORE_DEMO_REDIS_HOST") or None
    REDIS_PORT = _optional_int("MCPSTORE_DEMO_REDIS_PORT", 6379)
    REDIS_PASSWORD = os.getenv("MCPSTORE_DEMO_REDIS_PASSWORD") or None
    REDIS_NAMESPACE = os.getenv("MCPSTORE_DEMO_REDIS_NAMESPACE", "mcpstore_demo")
    REDIS_DB = _optional_int("MCPSTORE_DEMO_REDIS_DB", 0)

    MCP_SERVICE_URL = os.getenv(
        "MCPSTORE_DEMO_SERVICE_URL", "https://example.com/mcp"
    )

    DEEPSEEK_API_KEY = os.getenv("MCPSTORE_DEMO_DEEPSEEK_API_KEY", "")
    DEEPSEEK_API_BASE = os.getenv(
        "MCPSTORE_DEMO_DEEPSEEK_API_BASE", "https://api.deepseek.com"
    )
    DEEPSEEK_MODEL = os.getenv(
        "MCPSTORE_DEMO_DEEPSEEK_MODEL", "deepseek-chat"
    )

    QWEN_API_KEY = os.getenv("MCPSTORE_DEMO_QWEN_API_KEY", "")
    QWEN_API_BASE = os.getenv("MCPSTORE_DEMO_QWEN_API_BASE", "")
    QWEN_MODEL = os.getenv("MCPSTORE_DEMO_QWEN_MODEL", "")

    GLM_API_KEY = os.getenv("MCPSTORE_DEMO_GLM_API_KEY", "")
    GLM_API_BASE = os.getenv(
        "MCPSTORE_DEMO_GLM_API_BASE", "https://open.bigmodel.cn/api/paas/v4"
    )
    GLM_MODEL = os.getenv("MCPSTORE_DEMO_GLM_MODEL", "")

    ACTIVE_PROVIDER = os.getenv("MCPSTORE_DEMO_LLM_PROVIDER", "").lower()

    @classmethod
    def get_redis_config(cls):
        """Return Redis settings, failing only when a Redis example uses them."""
        if not cls.REDIS_HOST:
            raise RuntimeError(
                "Redis demo configuration is missing. Copy .env.example to .env "
                "and set MCPSTORE_DEMO_REDIS_HOST."
            )
        return {
            "host": cls.REDIS_HOST,
            "port": cls.REDIS_PORT,
            "password": cls.REDIS_PASSWORD,
            "namespace": cls.REDIS_NAMESPACE,
            "db": cls.REDIS_DB,
        }

    @classmethod
    def get_llm_config(cls):
        """Return the explicitly selected LLM provider configuration."""
        providers = {
            "deepseek": {
                "api_key": cls.DEEPSEEK_API_KEY,
                "api_base": cls.DEEPSEEK_API_BASE,
                "model": cls.DEEPSEEK_MODEL,
            },
            "glm": {
                "api_key": cls.GLM_API_KEY,
                "api_base": cls.GLM_API_BASE,
                "model": cls.GLM_MODEL,
            },
            "qwen": {
                "api_key": cls.QWEN_API_KEY,
                "api_base": cls.QWEN_API_BASE,
                "model": cls.QWEN_MODEL,
            },
        }
        config = providers.get(cls.ACTIVE_PROVIDER)
        if not config or not all(config.values()):
            raise RuntimeError(
                "LLM demo configuration is missing. Set MCPSTORE_DEMO_LLM_PROVIDER "
                "and the matching provider values in .env."
            )
        return config


def setup_example_import():
    """Verify that this interpreter has an installed MCPStore package."""
    try:
        import mcpstore
        import mcpstore._rust as rust
    except ImportError as error:
        raise RuntimeError(
            "mcpstore is not installed in this Python environment. Run "
            "./run_debug.sh and choose menu option 7 first."
        ) from error

    print(f"[+] Using installed MCPStore: {mcpstore.__file__}")
    print(f"[+] Using Rust extension: {rust.__file__}")
    return True


def print_section_header(title, width=60):
    """Print a formatted section header."""
    print("=" * width)
    print(f"🔧 {title}")
    print("=" * width)


def print_test_header(title, width=50):
    """Print a formatted test header."""
    print("\n" + "🧪 " + "=" * (width - 3))
    print(f"🧪 {title}")
    print("🧪 " + "=" * (width - 3))


def print_result(label, result, width=30):
    """Print a formatted result line."""
    print(f"   {label:<{width}}: {result}")


def print_step(step_num, description, total_steps=None):
    """Print a formatted step marker."""
    if total_steps:
        print(f"📍 Step {step_num}/{total_steps}: {description}")
    else:
        print(f"📍 Step {step_num}: {description}")


def cleanup_store(store, title="Config Cleanup"):
    """Reset the store configuration."""
    print_section_header(title)
    try:
        services_before = store.for_store().list_services()
        print(f"   Current service count: {len(services_before)}")
        print("   Resetting configuration...")
        store.for_store().reset_config()
        print("    Configuration reset complete")
    except Exception as error:
        print(f"   ⚠️ Cleanup failed: {error}")


def get_service_config(service_name, service_type="local", **kwargs):
    """Return a standard service configuration template."""
    if service_type == "local":
        return {
            "name": service_name,
            "command": "uvx",
            "args": ["gitodo"],
        }
    if service_type == "remote":
        return {
            "name": service_name,
            "url": kwargs.get("url", ExampleConfig.MCP_SERVICE_URL),
            "auth": kwargs.get("auth"),
            "headers": kwargs.get("headers", {}),
        }
    if service_type == "playwright":
        return {
            "name": service_name,
            "command": "npx",
            "args": ["@playwright/mcp"],
        }
    raise ValueError(f"Unsupported service type: {service_type}")


def wait_for_service(store, service_name, timeout=10):
    """Wait until the service is ready."""
    print(f"   Waiting for service {service_name}...")
    try:
        result = store.for_store().wait_service(
            service_name=service_name, timeout=timeout
        )
        print(f"    Service {service_name} is ready")
        return result
    except Exception as error:
        print(f"   ❌ Service {service_name} failed to become ready: {error}")
        return None


def demo_mcp_config():
    """Return the demo MCP configuration."""
    return {
        "mcpServers": {
            "playwright": {
                "command": "npx",
                "args": ["@playwright/mcp"],
            },
            "weather": {
                "url": ExampleConfig.MCP_SERVICE_URL,
                "auth": "demo-token",
            },
        }
    }


auto_setup_import = setup_example_import
setup_import_path = setup_example_import
