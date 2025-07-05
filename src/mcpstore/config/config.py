import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import logging
from typing import Dict, Any

logger = logging.getLogger(__name__)

class LoggingConfig:
    """日志配置管理器"""

    _debug_enabled = False
    _configured = False

    @classmethod
    def setup_logging(cls, debug: bool = False, force_reconfigure: bool = False):
        """
        设置日志配置

        Args:
            debug: 是否启用调试日志
            force_reconfigure: 是否强制重新配置
        """
        if cls._configured and not force_reconfigure:
            # 如果已经配置过且不强制重新配置，只更新日志级别
            if debug != cls._debug_enabled:
                cls._set_log_level(debug)
            return

        # 配置日志格式
        if debug:
            log_format = '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
            log_level = logging.DEBUG
        else:
            log_format = '%(levelname)s - %(message)s'
            log_level = logging.ERROR  # 非调试模式只显示错误

        # 获取根日志器
        root_logger = logging.getLogger()

        # 清除现有的处理器
        for handler in root_logger.handlers[:]:
            root_logger.removeHandler(handler)

        # 创建新的处理器
        handler = logging.StreamHandler()
        formatter = logging.Formatter(log_format)
        handler.setFormatter(formatter)

        # 设置日志级别
        root_logger.setLevel(log_level)
        handler.setLevel(log_level)

        # 添加处理器
        root_logger.addHandler(handler)

        # 设置特定模块的日志级别
        cls._configure_module_loggers(debug)

        cls._debug_enabled = debug
        cls._configured = True

    @classmethod
    def _set_log_level(cls, debug: bool):
        """设置日志级别"""
        if debug:
            log_level = logging.DEBUG
        else:
            log_level = logging.ERROR  # 非调试模式只显示错误

        # 更新根日志器级别
        root_logger = logging.getLogger()
        root_logger.setLevel(log_level)

        # 更新所有处理器级别
        for handler in root_logger.handlers:
            handler.setLevel(log_level)

        # 更新特定模块的日志级别
        cls._configure_module_loggers(debug)

        cls._debug_enabled = debug

    @classmethod
    def _configure_module_loggers(cls, debug: bool):
        """配置特定模块的日志器"""
        if debug:
            # 调试模式：显示所有 MCPStore 相关日志
            mcpstore_loggers = [
                'mcpstore',
                'mcpstore.core',
                'mcpstore.core.store',
                'mcpstore.core.context',
                'mcpstore.core.orchestrator',
                'mcpstore.core.registry',
                'mcpstore.core.client_manager',
                'mcpstore.core.session_manager',
                'mcpstore.core.tool_resolver',
                'mcpstore.plugins.json_mcp',
                'mcpstore.adapters.langchain_adapter'
            ]

            for logger_name in mcpstore_loggers:
                module_logger = logging.getLogger(logger_name)
                module_logger.setLevel(logging.DEBUG)
        else:
            # 非调试模式：只显示警告和错误
            mcpstore_loggers = [
                'mcpstore',
                'mcpstore.core',
                'mcpstore.core.store',
                'mcpstore.core.context',
                'mcpstore.core.orchestrator',
                'mcpstore.core.registry',
                'mcpstore.core.client_manager',
                'mcpstore.core.session_manager',
                'mcpstore.core.tool_resolver',
                'mcpstore.plugins.json_mcp',
                'mcpstore.adapters.langchain_adapter'
            ]

            for logger_name in mcpstore_loggers:
                module_logger = logging.getLogger(logger_name)
                module_logger.setLevel(logging.ERROR)  # 非调试模式只显示错误

    @classmethod
    def is_debug_enabled(cls) -> bool:
        """检查是否启用了调试模式"""
        return cls._debug_enabled

    @classmethod
    def enable_debug(cls):
        """启用调试模式"""
        cls.setup_logging(debug=True, force_reconfigure=True)

    @classmethod
    def disable_debug(cls):
        """禁用调试模式"""
        cls.setup_logging(debug=False, force_reconfigure=True)

# --- Configuration Constants (default values) ---
# 核心监控配置
HEARTBEAT_INTERVAL_SECONDS = 60  # 心跳检查间隔（秒）
HTTP_TIMEOUT_SECONDS = 10        # HTTP请求超时（秒）
RECONNECTION_INTERVAL_SECONDS = 60  # 重连尝试间隔（秒）

# HTTP端点配置
STREAMABLE_HTTP_ENDPOINT = "/mcp"  # 流式HTTP端点路径

# @dataclass
# class LLMConfig:
#     provider: str = "openai_compatible"
#     api_key: str = ""
#     model: str = ""
#     base_url: Optional[str] = None

# def load_llm_config() -> LLMConfig:
#     """从环境变量加载LLM配置（仅支持openai兼容接口）"""
#     api_key = os.environ.get("OPENAI_API_KEY", "")
#     model = os.environ.get("OPENAI_MODEL", "")
#     base_url = os.environ.get("OPENAI_BASE_URL")
#     provider = "openai_compatible"
#     if not api_key:
#         logger.warning("OPENAI_API_KEY not set in environment.")
#     if not model:
#         logger.warning("OPENAI_MODEL not set in environment.")
#     return LLMConfig(provider=provider, api_key=api_key, model=model, base_url=base_url)

def _get_env_int(var: str, default: int) -> int:
    try:
        return int(os.environ.get(var, default))
    except Exception:
        logger.warning(f"环境变量{var}格式错误，使用默认值{default}")
        return default

def _get_env_bool(var: str, default: bool) -> bool:
    val = os.environ.get(var)
    if val is None:
        return default
    return val.lower() in ("1", "true", "yes", "on")

def load_app_config() -> Dict[str, Any]:
    """从环境变量加载全局配置"""
    config_data = {
        # 核心监控配置
        "heartbeat_interval": _get_env_int("HEARTBEAT_INTERVAL_SECONDS", HEARTBEAT_INTERVAL_SECONDS),
        "http_timeout": _get_env_int("HTTP_TIMEOUT_SECONDS", HTTP_TIMEOUT_SECONDS),
        "reconnection_interval": _get_env_int("RECONNECTION_INTERVAL_SECONDS", RECONNECTION_INTERVAL_SECONDS),

        # HTTP端点配置
        "streamable_http_endpoint": os.environ.get("STREAMABLE_HTTP_ENDPOINT", STREAMABLE_HTTP_ENDPOINT),
    }
    # 加载LLM配置
    # config_data["llm_config"] = load_llm_config()
    # logger.info(f"Loaded configuration from environment: {config_data}")
    return config_data
