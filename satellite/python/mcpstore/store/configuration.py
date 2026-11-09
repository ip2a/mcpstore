"""Definition and scope configuration operations."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.core.models import ScopeDescriptor, ScopeRef
from mcpstore.native.records import (
    _base_config_payload,
    _descriptor_payload,
    _dict_payload,
    _record_value,
    _scope_payload,
)

def namespace(backend) -> str:
    return str(backend._inner.namespace())

def current_store(store) -> str:
    return str(store._inner.current_store())

def load_from_config(backend) -> None:
    backend._inner.load_from_config()

def add_service(backend, service_name: str, config: Dict[str, Any]) -> None:
    backend._inner.add_service(service_name, _dict_payload(config, "Service config"))

def declare_service_scope(
    backend,
    service_name: str,
    scope: ScopeRef | Dict[str, Any],
    descriptor: ScopeDescriptor | Dict[str, Any],
) -> str:
    return str(
        backend._inner.declare_service_scope(
            service_name,
            _scope_payload(scope),
            _descriptor_payload(descriptor),
        )
    )

def remove_service_scope(
    backend,
    service_name: str,
    scope: ScopeRef | Dict[str, Any],
) -> None:
    backend._inner.remove_service_scope(service_name, _scope_payload(scope))

def patch_service(backend, service_name: str, base_updates: Dict[str, Any]) -> None:
    backend._inner.patch_service(
        service_name,
        _base_config_payload(base_updates, "Service config patch"),
    )

def update_service(backend, service_name: str, config: Dict[str, Any]) -> None:
    backend._inner.update_service(
        service_name,
        _base_config_payload(config, "Service config update"),
    )

def remove_service(backend, service_name: str) -> None:
    backend._inner.remove_service(service_name)

def get_definition_config(backend, service_name: str) -> Optional[Dict[str, Any]]:
    return _record_value(backend._inner.get_definition_config(service_name))

def get_effective_config(
    backend,
    service_name: str,
    scope: ScopeRef | Dict[str, Any],
) -> Optional[Dict[str, Any]]:
    return _record_value(
        backend._inner.get_effective_config(service_name, _scope_payload(scope))
    )

def show_config(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.show_config())

def show_scope_config(backend, scope: ScopeRef | Dict[str, Any]) -> Dict[str, Any]:
    return _record_value(backend._inner.show_scope_config(_scope_payload(scope)))

def reset_config(backend) -> None:
    backend._inner.reset_config()

def reset_scope(backend, scope: ScopeRef | Dict[str, Any]) -> None:
    backend._inner.reset_scope(_scope_payload(scope))
