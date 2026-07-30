"""Plain-dict conversion at the Rust binding boundary."""

from __future__ import annotations

from typing import Any, Dict

from pydantic import BaseModel, TypeAdapter

from mcpstore.core.models import ScopeDescriptor, ScopeRef


_SCOPE_ADAPTER: TypeAdapter[ScopeRef] = TypeAdapter(ScopeRef)


def _record_value(value: Any) -> Any:
    """Recursively normalize a serde-derived value into plain dict/list/scalars."""
    if isinstance(value, dict):
        return {key: _record_value(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_record_value(item) for item in value]
    return value


def _dict_payload(value: Any, context: str) -> Dict[str, Any]:
    if isinstance(value, BaseModel):
        value = value.model_dump(mode="json", exclude_none=False)
    if not isinstance(value, dict):
        raise TypeError(f"{context} must be a dictionary")
    return _record_value(value)


def _base_config_payload(value: Any, context: str) -> Dict[str, Any]:
    payload = _dict_payload(value, context)
    if "_mcpstore" in payload:
        raise ValueError(
            f"{context} only accepts base MCP fields; change scopes with "
            "declare_service_scope() or remove_service_scope()"
        )
    return payload


def _scope_payload(scope: ScopeRef | Dict[str, Any]) -> Dict[str, Any]:
    validated = _SCOPE_ADAPTER.validate_python(scope)
    return _SCOPE_ADAPTER.dump_python(validated, mode="json")


def _descriptor_payload(
    descriptor: ScopeDescriptor | Dict[str, Any],
) -> Dict[str, Any]:
    validated = ScopeDescriptor.model_validate(descriptor)
    return validated.model_dump(mode="json", exclude_none=False)


__all__ = [
    "_base_config_payload",
    "_descriptor_payload",
    "_dict_payload",
    "_record_value",
    "_scope_payload",
]
