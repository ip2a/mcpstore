"""Instance-owned tool transformation models and facade helpers."""

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
    validation_schema: Optional[Dict[str, Any]] = None
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
            "validation_schema": self.validation_schema,
        }
        return payload


@dataclass
class ToolTransformConfig:
    tool_name: str
    new_tool_name: Optional[str] = None
    new_description: Optional[str] = None
    argument_transforms: Dict[str, ArgumentTransform] = field(default_factory=dict)
    safety_policy: Optional[Dict[str, Any]] = None
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
            "safety_policy": self.safety_policy,
            "tags": list(self.tags),
            "enabled": self.enabled,
        }


class ToolTransformer:
    def __init__(self, context: Any = None, instance_id: Optional[str] = None):
        self._context = context
        self._instance_id = instance_id

    def bind(self, context: Any, instance_id: Optional[str] = None) -> "ToolTransformer":
        self._context = context
        self._instance_id = instance_id
        return self

    def register_transformation(
        self,
        config: ToolTransformConfig,
        instance_id: Optional[str] = None,
    ) -> str:
        context = self._require_context()
        resolved_instance = instance_id or self._instance_id
        if not resolved_instance:
            raise ValueError("Tool transformation registration requires instance_id")
        rule = context.set_tool_transform(
            resolved_instance,
            config.tool_name,
            config.to_rust_payload(),
        )
        return str(rule.get("display_name") or config.new_tool_name or config.tool_name)

    def create_llm_friendly_tool(
        self,
        tool_name: str,
        friendly_name: Optional[str] = None,
        simplified_description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_checks: bool = True,
        instance_id: Optional[str] = None,
    ) -> str:
        context = self._require_context()
        resolved_instance = instance_id or self._instance_id
        if not resolved_instance:
            raise ValueError("Tool transformation registration requires instance_id")
        rule = context.create_llm_friendly_tool_transform(
            resolved_instance,
            tool_name,
            friendly_name=friendly_name,
            description=simplified_description,
            hide_technical_params=hide_technical_params,
            add_safety_policy=add_safety_checks,
        )
        return str(rule.get("display_name") or friendly_name or f"{tool_name}_simple")

    @staticmethod
    def _default_safety_policy() -> Dict[str, Any]:
        return {
            "reject_dangerous_argument_names": True,
            "dangerous_argument_name_patterns": [
                "__",
                "eval",
                "exec",
                "import",
                "open",
                "file",
            ],
        }

    def create_parameter_renamed_tool(
        self,
        tool_name: str,
        parameter_mapping: Dict[str, str],
        new_tool_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> str:
        context = self._require_context()
        resolved_instance = instance_id or self._instance_id
        if not resolved_instance:
            raise ValueError("Tool transformation registration requires instance_id")
        rule = context.create_parameter_renamed_tool_transform(
            resolved_instance,
            tool_name,
            parameter_mapping,
            new_tool_name=new_tool_name,
        )
        return str(rule.get("display_name") or new_tool_name or f"{tool_name}_renamed")

    def create_validated_tool(
        self,
        tool_name: str,
        validation_rules: Dict[str, Any],
        new_tool_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> str:
        for param_name, validation_rule in validation_rules.items():
            if callable(validation_rule):
                raise NotImplementedError(
                    "Callable validation rules are not cross-process serializable; "
                    "use JSON-schema validation rules for Rust-backed transforms."
                )
            if not isinstance(validation_rule, dict):
                raise TypeError("Tool validation rules must be JSON-schema dictionaries")
        context = self._require_context()
        resolved_instance = instance_id or self._instance_id
        if not resolved_instance:
            raise ValueError("Tool transformation registration requires instance_id")
        rule = context.create_validated_tool_transform(
            resolved_instance,
            tool_name,
            validation_rules,
            new_tool_name=new_tool_name,
        )
        return str(rule.get("display_name") or new_tool_name or f"{tool_name}_validated")

    def get_transformation_config(
        self,
        tool_name: str,
        instance_id: Optional[str] = None,
    ) -> Optional[Dict[str, Any]]:
        context = self._require_context()
        resolved_instance = instance_id or self._instance_id
        if not resolved_instance:
            raise ValueError("Tool transformation lookup requires instance_id")
        return context.get_tool_transform(resolved_instance, tool_name)

    def list_transformed_tools(self) -> List[str]:
        context = self._require_context()
        return [
            str(rule.get("display_name") or rule.get("tool_name"))
            for rule in context.list_tool_transforms()
        ]

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
    def __init__(self, context: Any = None, instance_id: Optional[str] = None):
        self.transformer = ToolTransformer(context, instance_id)

    def bind(self, context: Any, instance_id: Optional[str] = None) -> "ToolTransformationManager":
        self.transformer.bind(context, instance_id)
        return self

    def create_simple_weather_tool(self, tool_name: str, instance_id: Optional[str] = None) -> str:
        return self.transformer.create_llm_friendly_tool(
            tool_name=tool_name,
            friendly_name="get_weather",
            simplified_description="Get current weather for a city. Just provide the city name.",
            hide_technical_params=True,
            add_safety_checks=False,
            instance_id=instance_id,
        )

    def create_user_friendly_api_tool(
        self,
        tool_name: str,
        api_type: str,
        instance_id: Optional[str] = None,
    ) -> str:
        friendly_names = {
            "weather": "check_weather",
            "news": "get_news",
            "search": "search_web",
            "translate": "translate_text",
            "image": "process_image",
        }
        return self.transformer.create_llm_friendly_tool(
            tool_name=tool_name,
            friendly_name=friendly_names.get(api_type, f"use_{api_type}"),
            simplified_description=f"Easy-to-use {api_type} tool with simplified parameters.",
            hide_technical_params=True,
            add_safety_checks=False,
            instance_id=instance_id,
        )

    def enable_transformation(
        self,
        instance_id: str,
        tool_name: str,
        enabled: bool = True,
    ) -> Dict[str, Any]:
        existing = self.transformer.get_transformation_config(tool_name, instance_id) or {}
        return self.transformer._require_context().set_tool_transform(
            instance_id,
            tool_name,
            {
                "display_name": existing.get("display_name"),
                "description": existing.get("description"),
                "arguments": existing.get("arguments") or [],
                "safety_policy": existing.get("safety_policy"),
                "tags": existing.get("tags") or [],
                "enabled": enabled,
            },
        )

    def is_transformation_enabled(self, instance_id: str, tool_name: str) -> bool:
        existing = self.transformer.get_transformation_config(tool_name, instance_id)
        return bool(existing.get("enabled", True)) if existing else False

    def get_transformation_summary(self) -> Dict[str, Any]:
        context = self.transformer._require_context()
        rules = context.list_tool_transforms()
        return {
            "total_transformations": len(rules),
            "enabled_transformations": sum(1 for rule in rules if rule.get("enabled")),
            "available_tools": [
                rule.get("display_name") or rule.get("tool_name") for rule in rules
            ],
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
