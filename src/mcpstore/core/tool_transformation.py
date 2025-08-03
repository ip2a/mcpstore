#!/usr/bin/env python3
"""
Tool Transformation Functionality
Based on FastMCP 2.8 tool transformation capabilities, providing LLM-friendly tool interfaces
"""

import logging
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Any, Optional, Callable

logger = logging.getLogger(__name__)

class TransformationType(Enum):
    """Transformation types"""
    RENAME_ARGS = "rename_args"           # Rename parameters
    HIDE_ARGS = "hide_args"               # Hide parameters
    MODIFY_DESCRIPTION = "modify_description"  # Modify description
    ADD_VALIDATION = "add_validation"     # Add validation
    SIMPLIFY_INTERFACE = "simplify_interface"  # Simplify interface
    ENHANCE_SAFETY = "enhance_safety"     # Enhance safety

@dataclass
class ArgumentTransform:
    """Argument transformation configuration"""
    original_name: str
    new_name: Optional[str] = None        # New parameter name
    hidden: bool = False                  # Whether to hide
    default_value: Any = None             # Default value
    description: Optional[str] = None     # New description
    validation_fn: Optional[Callable] = None  # Validation function
    transform_fn: Optional[Callable] = None   # Transformation function

@dataclass
class ToolTransformConfig:
    """Tool transformation configuration"""
    original_tool_name: str
    new_tool_name: Optional[str] = None
    new_description: Optional[str] = None
    argument_transforms: Dict[str, ArgumentTransform] = field(default_factory=dict)
    pre_execution_hooks: List[Callable] = field(default_factory=list)
    post_execution_hooks: List[Callable] = field(default_factory=list)
    tags: List[str] = field(default_factory=list)
    enabled: bool = True

class ToolTransformer:
    """Tool transformer"""
    
    def __init__(self):
        self._transformations: Dict[str, ToolTransformConfig] = {}
        self._original_tools: Dict[str, Any] = {}
    
    def register_transformation(self, config: ToolTransformConfig) -> str:
        """
        注册工具转换配置
        
        Args:
            config: 转换配置
            
        Returns:
            str: 转换后的工具名称
        """
        transformed_name = config.new_tool_name or f"{config.original_tool_name}_enhanced"
        self._transformations[transformed_name] = config
        
        logger.info(f"Registered tool transformation: {config.original_tool_name} -> {transformed_name}")
        return transformed_name
    
    def create_llm_friendly_tool(
        self,
        original_tool_name: str,
        friendly_name: Optional[str] = None,
        simplified_description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_checks: bool = True
    ) -> str:
        """
        创建 LLM 友好的工具版本
        
        Args:
            original_tool_name: 原始工具名
            friendly_name: 友好名称
            simplified_description: 简化描述
            hide_technical_params: 是否隐藏技术参数
            add_safety_checks: 是否添加安全检查
            
        Returns:
            str: 转换后的工具名称
        """
        config = ToolTransformConfig(
            original_tool_name=original_tool_name,
            new_tool_name=friendly_name or f"{original_tool_name}_simple",
            new_description=simplified_description,
            tags=["llm-friendly", "simplified"]
        )
        
        if hide_technical_params:
            # 隐藏常见的技术参数
            technical_params = ["timeout", "retry_count", "debug", "verbose", "raw_output"]
            for param in technical_params:
                config.argument_transforms[param] = ArgumentTransform(
                    original_name=param,
                    hidden=True,
                    default_value=self._get_default_for_param(param)
                )
        
        if add_safety_checks:
            # 添加安全检查钩子
            config.pre_execution_hooks.append(self._safety_check_hook)
        
        return self.register_transformation(config)
    
    def create_parameter_renamed_tool(
        self,
        original_tool_name: str,
        parameter_mapping: Dict[str, str],
        new_tool_name: Optional[str] = None
    ) -> str:
        """
        创建参数重命名的工具版本
        
        Args:
            original_tool_name: 原始工具名
            parameter_mapping: 参数映射 {原参数名: 新参数名}
            new_tool_name: 新工具名
            
        Returns:
            str: 转换后的工具名称
        """
        config = ToolTransformConfig(
            original_tool_name=original_tool_name,
            new_tool_name=new_tool_name or f"{original_tool_name}_renamed",
            tags=["parameter-renamed"]
        )
        
        for original_param, new_param in parameter_mapping.items():
            config.argument_transforms[original_param] = ArgumentTransform(
                original_name=original_param,
                new_name=new_param
            )
        
        return self.register_transformation(config)
    
    def create_validated_tool(
        self,
        original_tool_name: str,
        validation_rules: Dict[str, Callable],
        new_tool_name: Optional[str] = None
    ) -> str:
        """
        创建带验证的工具版本
        
        Args:
            original_tool_name: 原始工具名
            validation_rules: 验证规则 {参数名: 验证函数}
            new_tool_name: 新工具名
            
        Returns:
            str: 转换后的工具名称
        """
        config = ToolTransformConfig(
            original_tool_name=original_tool_name,
            new_tool_name=new_tool_name or f"{original_tool_name}_validated",
            tags=["validated", "safe"]
        )
        
        for param_name, validation_fn in validation_rules.items():
            config.argument_transforms[param_name] = ArgumentTransform(
                original_name=param_name,
                validation_fn=validation_fn
            )
        
        return self.register_transformation(config)
    
    def get_transformation_config(self, tool_name: str) -> Optional[ToolTransformConfig]:
        """获取工具转换配置"""
        return self._transformations.get(tool_name)
    
    def list_transformed_tools(self) -> List[str]:
        """列出所有转换后的工具"""
        return list(self._transformations.keys())
    
    def _get_default_for_param(self, param_name: str) -> Any:
        """获取参数的默认值"""
        defaults = {
            "timeout": 30.0,
            "retry_count": 3,
            "debug": False,
            "verbose": False,
            "raw_output": False
        }
        return defaults.get(param_name)
    
    def _safety_check_hook(self, tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
        """安全检查钩子"""
        # 基本的安全检查
        if not isinstance(args, dict):
            raise ValueError("Arguments must be a dictionary")
        
        # 检查危险参数
        dangerous_keys = ["__", "eval", "exec", "import", "open", "file"]
        for key in args:
            if any(dangerous in str(key).lower() for dangerous in dangerous_keys):
                logger.warning(f"Potentially dangerous parameter detected: {key}")
        
        return args

class ToolTransformationManager:
    """工具转换管理器"""
    
    def __init__(self):
        self.transformer = ToolTransformer()
        self._enabled_transformations: Dict[str, bool] = {}
    
    def create_simple_weather_tool(self, original_tool_name: str) -> str:
        """创建简化的天气工具"""
        return self.transformer.create_llm_friendly_tool(
            original_tool_name=original_tool_name,
            friendly_name="get_weather",
            simplified_description="Get current weather for a city. Just provide the city name.",
            hide_technical_params=True,
            add_safety_checks=True
        )
    
    def create_user_friendly_api_tool(self, original_tool_name: str, api_type: str) -> str:
        """创建用户友好的 API 工具"""
        friendly_names = {
            "weather": "check_weather",
            "news": "get_news",
            "search": "search_web",
            "translate": "translate_text",
            "image": "process_image"
        }
        
        return self.transformer.create_llm_friendly_tool(
            original_tool_name=original_tool_name,
            friendly_name=friendly_names.get(api_type, f"use_{api_type}"),
            simplified_description=f"Easy-to-use {api_type} tool with simplified parameters.",
            hide_technical_params=True,
            add_safety_checks=True
        )
    
    def enable_transformation(self, tool_name: str, enabled: bool = True):
        """启用/禁用工具转换"""
        self._enabled_transformations[tool_name] = enabled
        logger.info(f"Tool transformation {tool_name} {'enabled' if enabled else 'disabled'}")
    
    def is_transformation_enabled(self, tool_name: str) -> bool:
        """检查工具转换是否启用"""
        return self._enabled_transformations.get(tool_name, True)
    
    def get_transformation_summary(self) -> Dict[str, Any]:
        """获取转换摘要"""
        return {
            "total_transformations": len(self.transformer._transformations),
            "enabled_transformations": sum(1 for enabled in self._enabled_transformations.values() if enabled),
            "available_tools": self.transformer.list_transformed_tools(),
            "transformation_types": [
                "llm-friendly",
                "parameter-renamed", 
                "validated",
                "simplified"
            ]
        }

# 全局实例
_global_transformation_manager = None

def get_transformation_manager() -> ToolTransformationManager:
    """获取全局工具转换管理器"""
    global _global_transformation_manager
    if _global_transformation_manager is None:
        _global_transformation_manager = ToolTransformationManager()
    return _global_transformation_manager
