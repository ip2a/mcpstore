"""
MCPStore Web界面配置文件
"""

import os
from typing import Dict, Any

class WebConfig:
    """Web界面配置类"""
    
    # 应用基本信息
    APP_NAME = "MCPStore 管理面板"
    APP_VERSION = "v2.0.0"
    APP_DESCRIPTION = "增强版 - 更丝滑的管理体验"
    
    # Streamlit配置
    STREAMLIT_CONFIG = {
        "page_title": APP_NAME,
        "page_icon": "🚀",
        "layout": "wide",
        "initial_sidebar_state": "expanded"
    }
    
    # API配置
    DEFAULT_API_BASE_URL = "http://localhost:18611"
    DEFAULT_BACKEND_TYPE = "http"
    API_TIMEOUT = 10
    API_RETRY_COUNT = 3
    
    # UI配置
    UI_CONFIG = {
        "theme": "light",
        "auto_refresh": False,
        "refresh_interval": 5,
        "items_per_page": 10,
        "show_advanced_options": False,
        "enable_animations": True,
        "compact_mode": False
    }
    
    # 缓存配置
    CACHE_CONFIG = {
        "default_ttl": 30,  # 默认缓存时间（秒）
        "service_data_ttl": 30,
        "tool_data_ttl": 60,
        "monitoring_data_ttl": 5,
        "max_cache_size": 100
    }
    
    # 预设服务配置
    PRESET_SERVICES = [
        {
            "name": "mcpstore-wiki",
            "url": "http://59.110.160.18:21923/mcp",
            "description": "MCPStore官方Wiki服务",
            "category": "官方",
            "transport": "auto",
            "featured": True
        },
        {
            "name": "mcpstore-demo",
            "url": "http://59.110.160.18:21924/mcp",
            "description": "MCPStore演示服务",
            "category": "演示",
            "transport": "auto",
            "featured": True
        }
    ]
    
    # 监控配置
    MONITORING_CONFIG = {
        "enable_notifications": True,
        "alert_thresholds": {
            "service_health": 80,
            "response_time": 5000,
            "error_rate": 10
        },
        "notification_settings": {
            "auto_dismiss_time": 5,  # 秒
            "max_notifications": 10
        }
    }
    
    # 日志配置
    LOGGING_CONFIG = {
        "level": "INFO",
        "format": "%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        "max_history": 100
    }
    
    # 安全配置
    SECURITY_CONFIG = {
        "enable_auth": False,  # 暂时禁用认证
        "session_timeout": 3600,  # 1小时
        "max_login_attempts": 3
    }
    
    # 功能开关
    FEATURE_FLAGS = {
        "enable_direct_backend": True,
        "enable_service_wizard": True,
        "enable_realtime_monitoring": True,
        "enable_batch_operations": True,
        "enable_config_export": True,
        "enable_operation_history": True,
        "enable_advanced_search": True,
        "enable_service_templates": True
    }
    
    # 开发配置
    DEV_CONFIG = {
        "debug_mode": False,
        "show_debug_info": False,
        "enable_mock_data": False,
        "log_api_calls": True
    }
    
    @classmethod
    def get_config(cls, section: str = None) -> Dict[str, Any]:
        """获取配置"""
        if section:
            return getattr(cls, section.upper() + "_CONFIG", {})
        
        return {
            "app": {
                "name": cls.APP_NAME,
                "version": cls.APP_VERSION,
                "description": cls.APP_DESCRIPTION
            },
            "streamlit": cls.STREAMLIT_CONFIG,
            "api": {
                "base_url": cls.DEFAULT_API_BASE_URL,
                "backend_type": cls.DEFAULT_BACKEND_TYPE,
                "timeout": cls.API_TIMEOUT,
                "retry_count": cls.API_RETRY_COUNT
            },
            "ui": cls.UI_CONFIG,
            "cache": cls.CACHE_CONFIG,
            "preset_services": cls.PRESET_SERVICES,
            "monitoring": cls.MONITORING_CONFIG,
            "logging": cls.LOGGING_CONFIG,
            "security": cls.SECURITY_CONFIG,
            "features": cls.FEATURE_FLAGS,
            "dev": cls.DEV_CONFIG
        }
    
    @classmethod
    def is_feature_enabled(cls, feature: str) -> bool:
        """检查功能是否启用"""
        return cls.FEATURE_FLAGS.get(feature, False)
    
    @classmethod
    def get_preset_services(cls) -> list:
        """获取预设服务"""
        return cls.PRESET_SERVICES
    
    @classmethod
    def get_featured_services(cls) -> list:
        """获取推荐服务"""
        return [s for s in cls.PRESET_SERVICES if s.get('featured', False)]

class EnvironmentConfig:
    """环境配置类"""
    
    @staticmethod
    def get_env_config() -> Dict[str, Any]:
        """从环境变量获取配置"""
        return {
            "api_base_url": os.getenv("MCPSTORE_API_URL", WebConfig.DEFAULT_API_BASE_URL),
            "backend_type": os.getenv("MCPSTORE_BACKEND_TYPE", WebConfig.DEFAULT_BACKEND_TYPE),
            "debug_mode": os.getenv("MCPSTORE_DEBUG", "false").lower() == "true",
            "log_level": os.getenv("MCPSTORE_LOG_LEVEL", "INFO"),
            "enable_auth": os.getenv("MCPSTORE_ENABLE_AUTH", "false").lower() == "true"
        }
    
    @staticmethod
    def apply_env_config():
        """应用环境变量配置"""
        env_config = EnvironmentConfig.get_env_config()
        
        # 更新WebConfig
        WebConfig.DEFAULT_API_BASE_URL = env_config["api_base_url"]
        WebConfig.DEFAULT_BACKEND_TYPE = env_config["backend_type"]
        WebConfig.DEV_CONFIG["debug_mode"] = env_config["debug_mode"]
        WebConfig.LOGGING_CONFIG["level"] = env_config["log_level"]
        WebConfig.SECURITY_CONFIG["enable_auth"] = env_config["enable_auth"]

class ThemeConfig:
    """主题配置类"""
    
    LIGHT_THEME = {
        "primary_color": "#1f77b4",
        "background_color": "#ffffff",
        "secondary_background_color": "#f0f2f6",
        "text_color": "#262730"
    }
    
    DARK_THEME = {
        "primary_color": "#ff6b6b",
        "background_color": "#0e1117",
        "secondary_background_color": "#262730",
        "text_color": "#fafafa"
    }
    
    @classmethod
    def get_theme(cls, theme_name: str = "light") -> Dict[str, str]:
        """获取主题配置"""
        if theme_name == "dark":
            return cls.DARK_THEME
        return cls.LIGHT_THEME
    
    @classmethod
    def apply_theme(cls, theme_name: str = "light"):
        """应用主题"""
        theme = cls.get_theme(theme_name)
        
        # 这里可以设置Streamlit主题
        # 注意：Streamlit的主题设置需要在config.toml中配置
        return theme

# 初始化配置
def init_config():
    """初始化配置"""
    # 应用环境变量配置
    EnvironmentConfig.apply_env_config()
    
    # 返回完整配置
    return WebConfig.get_config()

# 导出配置实例
config = init_config()
