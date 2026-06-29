"""Rust-backed compatibility helpers for tool transformation rules."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable, Dict, List, Optional


class TransformationType(Enum):
    RENAME_ARGS = "rename_args"
    HIDE_ARGS = "hide_args"
    MODIFY_DESCRIPTION = "modify_description"
    ADD_VALIDATION = "add_validation"
    SIMPLIFY_INTERFACE = "simplify_interface"
    ENHANCE_SAFETY = "enhance_safety"


@dataclass
class ArgumentTransform:
    original_name: str
    new_name: Optional[str] = None
    hidden: bool = False
    default_value: Any = None
    description: Optional[str] = None
    validation_fn: Optional[Callable] = None
    transform_fn: Optional[Callable] = None

    def to_rust_payload(self) -> Dict[str, Any]:
        if self.validation_fn is not None or self.transform_fn is not None:
            raise NotImplementedError(
                "Callable argument hooks are not cross-process serializable; "
                "use Rust-backed declarative transforms only."
            )
        payload = {
            "original_name": self.original_name,
            "new_name": self.new_name,
            "hidden": self.hidden,
            "default_value": self.default_value,
            "description": self.description,
        }
        return payload


@dataclass
class ToolTransformConfig:
    original_tool_name: str
    new_tool_name: Optional[str] = None
    new_description: Optional[str] = None
    argument_transforms: Dict[str, ArgumentTransform] = field(default_factory=dict)
    pre_execution_hooks: List[Callable] = field(default_factory=list)
    post_execution_hooks: List[Callable] = field(default_factory=list)
    tags: List[str] = field(default_factory=list)
    enabled: bool = True

    def to_rust_payload(self) -> Dict[str, Any]:
        if self.pre_execution_hooks or self.post_execution_hooks:
            raise NotImplementedError(
                "Callable execution hooks are not cross-process serializable; "
                "shared tool transforms must be declarative."
            )
        return {
            "display_name": self.new_tool_name,
            "description": self.new_description,
            "arguments": [item.to_rust_payload() for item in self.argument_transforms.values()],
            "tags": list(self.tags),
            "enabled": self.enabled,
        }


class ToolTransformer:
    def __init__(self, context: Any = None, service_name: Optional[str] = None):
        self._context = context
        self._service_name = service_name

    def bind(self, context: Any, service_name: Optional[str] = None) -> "ToolTransformer":
        self._context = context
        self._service_name = service_name
        return self

    def register_transformation(
        self,
        config: ToolTransformConfig,
        service_name: Optional[str] = None,
    ) -> str:
        context = self._require_context()
        resolved_service = service_name or self._service_name
        if not resolved_service:
            raise ValueError("Tool transformation registration requires service_name")
        rule = context.set_tool_transform(
            resolved_service,
            config.original_tool_name,
            display_name=config.new_tool_name,
            description=config.new_description,
            arguments=[item.to_rust_payload() for item in config.argument_transforms.values()],
            tags=config.tags,
            enabled=config.enabled,
        )
        return str(rule.get("display_name") or config.new_tool_name or config.original_tool_name)

    def create_llm_friendly_tool(
        self,
        original_tool_name: str,
        friendly_name: Optional[str] = None,
        simplified_description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_checks: bool = False,
        service_name: Optional[str] = None,
    ) -> str:
        if add_safety_checks:
            raise NotImplementedError(
                "Python safety hooks are not part of the shared Rust-backed transform model."
            )
        config = ToolTransformConfig(
            original_tool_name=original_tool_name,
            new_tool_name=friendly_name or f"{original_tool_name}_simple",
            new_description=simplified_description,
            tags=["llm-friendly", "simplified"],
        )
        if hide_technical_params:
            for param in ["timeout", "retry_count", "debug", "verbose", "raw_output"]:
                config.argument_transforms[param] = ArgumentTransform(
                    original_name=param,
                    hidden=True,
                    default_value=self._get_default_for_param(param),
                )
        return self.register_transformation(config, service_name=service_name)

    def create_parameter_renamed_tool(
        self,
        original_tool_name: str,
        parameter_mapping: Dict[str, str],
        new_tool_name: Optional[str] = None,
        service_name: Optional[str] = None,
    ) -> str:
        config = ToolTransformConfig(
            original_tool_name=original_tool_name,
            new_tool_name=new_tool_name or f"{original_tool_name}_renamed",
            tags=["parameter-renamed"],
        )
        for original_param, new_param in parameter_mapping.items():
            config.argument_transforms[original_param] = ArgumentTransform(
                original_name=original_param,
                new_name=new_param,
            )
        return self.register_transformation(config, service_name=service_name)

    def create_validated_tool(self, *args, **kwargs) -> str:
        raise NotImplementedError(
            "Callable validation rules are not cross-process serializable; "
            "implement validation in the MCP service or a future Rust policy layer."
        )

    def get_transformation_config(
        self,
        tool_name: str,
        service_name: Optional[str] = None,
    ) -> Optional[Dict[str, Any]]:
        context = self._require_context()
        resolved_service = service_name or self._service_name
        if not resolved_service:
            raise ValueError("Tool transformation lookup requires service_name")
        return context.get_tool_transform(resolved_service, tool_name)

    def list_transformed_tools(self) -> List[str]:
        context = self._require_context()
        return [str(rule.get("display_name") or rule.get("original_tool_name")) for rule in context.list_tool_transforms()]

    def _require_context(self) -> Any:
        if self._context is None:
            raise RuntimeError(
                "ToolTransformer must be bound to a Rust-backed MCPStoreContext before use."
            )
        return self._context

    @staticmethod
    def _get_default_for_param(param_name: str) -> Any:
        return {
            "timeout": 30.0,
            "retry_count": 3,
            "debug": False,
            "verbose": False,
            "raw_output": False,
        }.get(param_name)


class ToolTransformationManager:
    def __init__(self, context: Any = None, service_name: Optional[str] = None):
        self.transformer = ToolTransformer(context, service_name)

    def bind(self, context: Any, service_name: Optional[str] = None) -> "ToolTransformationManager":
        self.transformer.bind(context, service_name)
        return self

    def create_simple_weather_tool(self, original_tool_name: str, service_name: Optional[str] = None) -> str:
        return self.transformer.create_llm_friendly_tool(
            original_tool_name=original_tool_name,
            friendly_name="get_weather",
            simplified_description="Get current weather for a city. Just provide the city name.",
            hide_technical_params=True,
            add_safety_checks=False,
            service_name=service_name,
        )

    def create_user_friendly_api_tool(
        self,
        original_tool_name: str,
        api_type: str,
        service_name: Optional[str] = None,
    ) -> str:
        friendly_names = {
            "weather": "check_weather",
            "news": "get_news",
            "search": "search_web",
            "translate": "translate_text",
            "image": "process_image",
        }
        return self.transformer.create_llm_friendly_tool(
            original_tool_name=original_tool_name,
            friendly_name=friendly_names.get(api_type, f"use_{api_type}"),
            simplified_description=f"Easy-to-use {api_type} tool with simplified parameters.",
            hide_technical_params=True,
            add_safety_checks=False,
            service_name=service_name,
        )

    def enable_transformation(
        self,
        service_name: str,
        tool_name: str,
        enabled: bool = True,
    ) -> Dict[str, Any]:
        existing = self.transformer.get_transformation_config(tool_name, service_name) or {}
        return self.transformer._require_context().set_tool_transform(
            service_name,
            tool_name,
            display_name=existing.get("display_name"),
            description=existing.get("description"),
            arguments=existing.get("arguments") or [],
            tags=existing.get("tags") or [],
            enabled=enabled,
        )

    def is_transformation_enabled(self, service_name: str, tool_name: str) -> bool:
        existing = self.transformer.get_transformation_config(tool_name, service_name)
        return bool(existing.get("enabled", True)) if existing else False

    def get_transformation_summary(self) -> Dict[str, Any]:
        context = self.transformer._require_context()
        rules = context.list_tool_transforms()
        return {
            "total_transformations": len(rules),
            "enabled_transformations": sum(1 for rule in rules if rule.get("enabled")),
            "available_tools": [rule.get("display_name") or rule.get("original_tool_name") for rule in rules],
            "transformation_types": ["llm-friendly", "parameter-renamed", "simplified"],
        }


_global_transformation_manager: Optional[ToolTransformationManager] = None


def get_transformation_manager() -> ToolTransformationManager:
    global _global_transformation_manager
    if _global_transformation_manager is None:
        _global_transformation_manager = ToolTransformationManager()
    return _global_transformation_manager


__all__ = [
    "ArgumentTransform",
    "ToolTransformConfig",
    "ToolTransformationManager",
    "ToolTransformer",
    "TransformationType",
    "get_transformation_manager",
]
