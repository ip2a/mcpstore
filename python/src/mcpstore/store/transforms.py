"""Instance-owned tool transform operations."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.native.records import _record_value

def set_tool_transform(
    backend,
    instance_id: str,
    tool_name: str,
    transform: Dict[str, Any],
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.set_tool_transform(instance_id, tool_name, transform)
    )

def create_llm_friendly_tool_transform(
    backend,
    instance_id: str,
    tool_name: str,
    *,
    friendly_name: Optional[str] = None,
    description: Optional[str] = None,
    hide_technical_params: bool = True,
    add_safety_policy: bool = True,
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.create_llm_friendly_tool_transform(
            instance_id,
            tool_name,
            friendly_name,
            description,
            hide_technical_params,
            add_safety_policy,
        )
    )

def create_parameter_renamed_tool_transform(
    backend,
    instance_id: str,
    tool_name: str,
    parameter_mapping: Dict[str, str],
    *,
    new_tool_name: Optional[str] = None,
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.create_parameter_renamed_tool_transform(
            instance_id,
            tool_name,
            parameter_mapping,
            new_tool_name,
        )
    )

def create_validated_tool_transform(
    backend,
    instance_id: str,
    tool_name: str,
    validation_rules: Dict[str, Any],
    *,
    new_tool_name: Optional[str] = None,
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.create_validated_tool_transform(
            instance_id,
            tool_name,
            validation_rules,
            new_tool_name,
        )
    )

def get_tool_transform(
    backend,
    instance_id: str,
    tool_name: str,
) -> Optional[Dict[str, Any]]:
    return _record_value(backend._inner.get_tool_transform(instance_id, tool_name))

def list_tool_transforms(backend) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_tool_transforms())

def delete_tool_transform(backend, instance_id: str, tool_name: str) -> None:
    backend._inner.delete_tool_transform(instance_id, tool_name)
