"""
统一监控配置管理器
处理用户监控配置，提供默认值和配置验证
现在从 MCPStoreConfig 获取配置值
"""

import logging
from typing import Dict, Any, Optional

from mcpstore.config.config_defaults import MonitoringConfigDefaults

logger = logging.getLogger(__name__)

_monitoring_defaults = MonitoringConfigDefaults()


class MonitoringConfigProcessor:
    """监控配置处理器"""

    @classmethod
    def get_config_from_mcpstore(cls) -> Dict[str, Any]:
        """
        从 MCPStoreConfig 获取监控配置

        Returns:
            从 MCPStoreConfig 读取的监控配置字典，如果 MCPStoreConfig 未初始化则返回默认配置
        """
        try:
            from mcpstore.config.toml_config import get_monitoring_config_with_defaults
            config = get_monitoring_config_with_defaults()

            # 将 dataclass 转换为字典格式以保持向后兼容
            if hasattr(config, '__dict__'):
                return {
                    "tools_update_hours": getattr(config, 'tools_update_hours', _monitoring_defaults.tools_update_hours),
                    "reconnection_seconds": getattr(config, 'reconnection_seconds', _monitoring_defaults.reconnection_seconds),
                    "cleanup_hours": getattr(config, 'cleanup_hours', _monitoring_defaults.cleanup_hours),
                    "enable_tools_update": getattr(config, 'enable_tools_update', _monitoring_defaults.enable_tools_update),
                    "enable_reconnection": getattr(config, 'enable_reconnection', _monitoring_defaults.enable_reconnection),
                    "update_tools_on_reconnection": getattr(config, 'update_tools_on_reconnection', _monitoring_defaults.update_tools_on_reconnection),
                    "detect_tools_changes": getattr(config, 'detect_tools_changes', _monitoring_defaults.detect_tools_changes),
                    "local_service_ping_timeout": getattr(config, 'local_service_ping_timeout', _monitoring_defaults.local_service_ping_timeout),
                    "remote_service_ping_timeout": getattr(config, 'remote_service_ping_timeout', _monitoring_defaults.remote_service_ping_timeout),
                    "enable_adaptive_timeout": getattr(config, 'enable_adaptive_timeout', _monitoring_defaults.enable_adaptive_timeout),
                    "adaptive_timeout_multiplier": getattr(config, 'adaptive_timeout_multiplier', _monitoring_defaults.adaptive_timeout_multiplier),
                    "response_time_history_size": getattr(config, 'response_time_history_size', _monitoring_defaults.response_time_history_size),
                }
            else:
                # 如果返回的是字典，直接使用
                return config

        except Exception as e:
            logger.warning(f"Failed to get monitoring config from MCPStoreConfig: {e}, using defaults")
            # 返回默认配置作为回退
            return cls._get_default_config()

    @classmethod
    def _get_default_config(cls) -> Dict[str, Any]:
        """获取默认监控配置（回退配置）"""
        return {
            "tools_update_hours": _monitoring_defaults.tools_update_hours,           # 2小时工具更新检查
            "reconnection_seconds": _monitoring_defaults.reconnection_seconds,        # 1分钟重连间隔
            "cleanup_hours": _monitoring_defaults.cleanup_hours,               # 24小时清理一次
            "enable_tools_update": _monitoring_defaults.enable_tools_update,       # 启用工具更新
            "enable_reconnection": _monitoring_defaults.enable_reconnection,       # 启用重连
            "update_tools_on_reconnection": _monitoring_defaults.update_tools_on_reconnection,  # 重连时更新工具
            "detect_tools_changes": _monitoring_defaults.detect_tools_changes,     # 关闭智能变化检测（避免额外开销）

            # 健康检查相关
            "local_service_ping_timeout": _monitoring_defaults.local_service_ping_timeout,   # 本地服务ping超时
            "remote_service_ping_timeout": _monitoring_defaults.remote_service_ping_timeout,  # 远程服务ping超时
            "enable_adaptive_timeout": _monitoring_defaults.enable_adaptive_timeout,   # 启用智能超时
            "adaptive_timeout_multiplier": _monitoring_defaults.adaptive_timeout_multiplier, # 智能超时倍数
            "response_time_history_size": _monitoring_defaults.response_time_history_size   # 响应时间历史大小
        }
    
    # 配置验证规则
    VALIDATION_RULES = {
        "tools_update_hours": {"min": 0.1, "max": 168},  # 6分钟到7天
        "reconnection_seconds": {"min": 10, "max": 600},
        "cleanup_hours": {"min": 1, "max": 168},
        "local_service_ping_timeout": {"min": 1, "max": 30},
        "remote_service_ping_timeout": {"min": 1, "max": 60},
        "adaptive_timeout_multiplier": {"min": 1.0, "max": 5.0},
        "response_time_history_size": {"min": 5, "max": 100}
    }
    
    @classmethod
    def process_config(cls, user_config: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """
        处理用户监控配置

        Args:
            user_config: 用户提供的监控配置（可选覆盖配置）

        Returns:
            完整的监控配置（基于 MCPStoreConfig + 用户覆盖）
        """
        if user_config is None:
            user_config = {}

        # 从 MCPStoreConfig 获取基础配置
        final_config = cls.get_config_from_mcpstore()

        # 应用用户配置覆盖
        for key, value in user_config.items():
            if key in final_config:
                # 验证配置值
                if cls._validate_config_value(key, value):
                    final_config[key] = value
                    logger.info(f"Applied user override for monitoring config: {key} = {value}")
                else:
                    logger.warning(f"Invalid monitoring config value for {key}: {value}, using MCPStoreConfig value: {final_config[key]}")
            else:
                logger.warning(f"Unknown monitoring config key: {key}, ignoring")

        # 配置一致性检查
        final_config = cls._ensure_config_consistency(final_config)
        
        logger.info(f"Monitoring configuration processed: {cls._get_config_summary(final_config)}")
        return final_config
    
    @classmethod
    def _validate_config_value(cls, key: str, value: Any) -> bool:
        """验证配置值"""
        try:
            # 布尔值配置
            if key.startswith("enable_") or key.startswith("update_") or key.startswith("detect_"):
                return isinstance(value, bool)
            
            # 数值配置
            if key in cls.VALIDATION_RULES:
                if not isinstance(value, (int, float)):
                    return False
                
                rules = cls.VALIDATION_RULES[key]
                return rules["min"] <= value <= rules["max"]
            
            return True
            
        except Exception as e:
            logger.error(f"Error validating config {key}={value}: {e}")
            return False
    
    @classmethod
    def _ensure_config_consistency(cls, config: Dict[str, Any]) -> Dict[str, Any]:
        """确保配置一致性"""
        # 如果禁用工具更新，相关配置无效
        if not config["enable_tools_update"]:
            config["update_tools_on_reconnection"] = False
            config["detect_tools_changes"] = False
        
        return config
    
    @classmethod
    def _get_config_summary(cls, config: Dict[str, Any]) -> str:
        """获取配置摘要"""
        return (f"tools_update={config['tools_update_hours']}h, "
                f"reconnection={config['reconnection_seconds']}s, "
                f"tools_update_enabled={config['enable_tools_update']}")
    
    @classmethod
    def convert_to_orchestrator_config(cls, monitoring_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        将监控配置转换为Orchestrator配置格式
        
        Args:
            monitoring_config: 处理后的监控配置
            
        Returns:
            Orchestrator兼容的配置
        """
        return {
            "timing": {
                "reconnection_interval_seconds": monitoring_config["reconnection_seconds"],
                "cleanup_interval_seconds": monitoring_config["cleanup_hours"] * 3600,
                "tools_update_interval_seconds": monitoring_config["tools_update_hours"] * 3600,
                "enable_tools_update": monitoring_config["enable_tools_update"],
                "update_tools_on_reconnection": monitoring_config["update_tools_on_reconnection"],
                "detect_tools_changes": monitoring_config["detect_tools_changes"],
                
                "local_service_ping_timeout": monitoring_config["local_service_ping_timeout"],
                "remote_service_ping_timeout": monitoring_config["remote_service_ping_timeout"],
                "enable_adaptive_timeout": monitoring_config["enable_adaptive_timeout"],
                "adaptive_timeout_multiplier": monitoring_config["adaptive_timeout_multiplier"],
                "response_time_history_size": monitoring_config["response_time_history_size"],
                
                # HTTP超时
                "http_timeout_seconds": max(
                    monitoring_config["local_service_ping_timeout"],
                    monitoring_config["remote_service_ping_timeout"]
                )
            }
        }
    
    @classmethod
    def get_default_config(cls) -> Dict[str, Any]:
        """获取默认配置（现在从 MCPStoreConfig 获取）"""
        return cls.get_config_from_mcpstore()
    
    @classmethod
    def validate_user_config(cls, user_config: Dict[str, Any]) -> tuple[bool, list[str]]:
        """
        验证用户配置

        Returns:
            (是否有效, 错误信息列表)
        """
        errors = []

        # 获取当前有效配置作为参考
        valid_config = cls.get_config_from_mcpstore()

        for key, value in user_config.items():
            if key not in valid_config:
                errors.append(f"Unknown config key: {key}")
            elif not cls._validate_config_value(key, value):
                rules = cls.VALIDATION_RULES.get(key, {})
                errors.append(f"Invalid value for {key}: {value} (expected: {rules})")

        return len(errors) == 0, errors
