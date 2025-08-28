"""
统一监控配置管理器
处理用户监控配置，提供默认值和配置验证
"""

import logging
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class MonitoringConfigProcessor:
    """监控配置处理器"""
    
    # 默认监控配置（推荐配置）
    DEFAULT_CONFIG = {
        "health_check_seconds": 30,        # 30秒健康检查
        "tools_update_hours": 2,           # 2小时工具更新检查
        "reconnection_seconds": 60,        # 1分钟重连间隔
        "cleanup_hours": 24,               # 24小时清理一次
        "enable_tools_update": True,       # 启用工具更新
        "enable_reconnection": True,       # 启用重连
        "update_tools_on_reconnection": True,  # 重连时更新工具
        "detect_tools_changes": False,     # 关闭智能变化检测（避免额外开销）
        
        # 健康检查相关
        "local_service_ping_timeout": 3,   # 本地服务ping超时
        "remote_service_ping_timeout": 5,  # 远程服务ping超时
        "startup_wait_time": 2,            # 启动等待时间
        "healthy_response_threshold": 1.0, # 健康响应阈值
        "warning_response_threshold": 3.0, # 警告响应阈值
        "slow_response_threshold": 10.0,   # 慢响应阈值
        "enable_adaptive_timeout": True,   # 启用智能超时
        "adaptive_timeout_multiplier": 2.0, # 智能超时倍数
        "response_time_history_size": 10   # 响应时间历史大小
    }
    
    # 配置验证规则
    VALIDATION_RULES = {
        "health_check_seconds": {"min": 10, "max": 300},
        "tools_update_hours": {"min": 0.1, "max": 168},  # 6分钟到7天
        "reconnection_seconds": {"min": 10, "max": 600},
        "cleanup_hours": {"min": 1, "max": 168},
        "local_service_ping_timeout": {"min": 1, "max": 30},
        "remote_service_ping_timeout": {"min": 1, "max": 60},
        "startup_wait_time": {"min": 0, "max": 30},
        "healthy_response_threshold": {"min": 0.1, "max": 10.0},
        "warning_response_threshold": {"min": 0.5, "max": 30.0},
        "slow_response_threshold": {"min": 1.0, "max": 120.0},
        "adaptive_timeout_multiplier": {"min": 1.0, "max": 5.0},
        "response_time_history_size": {"min": 5, "max": 100}
    }
    
    @classmethod
    def process_config(cls, user_config: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """
        处理用户监控配置
        
        Args:
            user_config: 用户提供的监控配置
            
        Returns:
            完整的监控配置
        """
        if user_config is None:
            user_config = {}
        
        # 从默认配置开始
        final_config = cls.DEFAULT_CONFIG.copy()
        
        # 应用用户配置
        for key, value in user_config.items():
            if key in cls.DEFAULT_CONFIG:
                # 验证配置值
                if cls._validate_config_value(key, value):
                    final_config[key] = value
                else:
                    logger.warning(f"Invalid monitoring config value for {key}: {value}, using default: {cls.DEFAULT_CONFIG[key]}")
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
        # 确保响应阈值的逻辑顺序
        if config["warning_response_threshold"] <= config["healthy_response_threshold"]:
            config["warning_response_threshold"] = config["healthy_response_threshold"] + 1.0
            logger.warning("Adjusted warning_response_threshold to maintain logical order")
        
        if config["slow_response_threshold"] <= config["warning_response_threshold"]:
            config["slow_response_threshold"] = config["warning_response_threshold"] + 2.0
            logger.warning("Adjusted slow_response_threshold to maintain logical order")
        
        # 如果禁用工具更新，相关配置无效
        if not config["enable_tools_update"]:
            config["update_tools_on_reconnection"] = False
            config["detect_tools_changes"] = False
        
        return config
    
    @classmethod
    def _get_config_summary(cls, config: Dict[str, Any]) -> str:
        """获取配置摘要"""
        return (f"health_check={config['health_check_seconds']}s, "
                f"tools_update={config['tools_update_hours']}h, "
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
                # 心跳和重连配置
                "heartbeat_interval_seconds": monitoring_config["health_check_seconds"],
                "reconnection_interval_seconds": monitoring_config["reconnection_seconds"],
                "cleanup_interval_seconds": monitoring_config["cleanup_hours"] * 3600,
                
                # 工具更新配置
                "tools_update_interval_seconds": monitoring_config["tools_update_hours"] * 3600,
                "enable_tools_update": monitoring_config["enable_tools_update"],
                "update_tools_on_reconnection": monitoring_config["update_tools_on_reconnection"],
                "detect_tools_changes": monitoring_config["detect_tools_changes"],
                
                # 健康检查配置
                "local_service_ping_timeout": monitoring_config["local_service_ping_timeout"],
                "remote_service_ping_timeout": monitoring_config["remote_service_ping_timeout"],
                "startup_wait_time": monitoring_config["startup_wait_time"],
                "healthy_response_threshold": monitoring_config["healthy_response_threshold"],
                "warning_response_threshold": monitoring_config["warning_response_threshold"],
                "slow_response_threshold": monitoring_config["slow_response_threshold"],
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
        """获取默认配置"""
        return cls.DEFAULT_CONFIG.copy()
    
    @classmethod
    def validate_user_config(cls, user_config: Dict[str, Any]) -> tuple[bool, list[str]]:
        """
        验证用户配置
        
        Returns:
            (是否有效, 错误信息列表)
        """
        errors = []
        
        for key, value in user_config.items():
            if key not in cls.DEFAULT_CONFIG:
                errors.append(f"Unknown config key: {key}")
            elif not cls._validate_config_value(key, value):
                rules = cls.VALIDATION_RULES.get(key, {})
                errors.append(f"Invalid value for {key}: {value} (expected: {rules})")
        
        return len(errors) == 0, errors
