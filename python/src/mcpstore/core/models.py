"""Public request models for the Rust-backed MCPStore Python API.

Scope references and service-config inputs live here. Every runtime
response is a plain ``dict`` produced by serializing Rust core types, so
no response envelope is needed at the SDK layer.
"""

from __future__ import annotations

from typing import Annotated, Any, Dict, List, Literal, Optional, Union

from pydantic import BaseModel, ConfigDict, Field


# ===== Scope =====

class RootScope(BaseModel):
    type: Literal["root"] = "root"

    model_config = ConfigDict(extra="forbid")


class StoreScope(BaseModel):
    type: Literal["store"] = "store"

    model_config = ConfigDict(extra="forbid")


class AgentScope(BaseModel):
    type: Literal["agent"] = "agent"
    agent_id: str

    model_config = ConfigDict(extra="forbid")


ScopeRef = Annotated[Union[StoreScope, AgentScope], Field(discriminator="type")]
ScopeView = Annotated[
    Union[RootScope, StoreScope, AgentScope], Field(discriminator="type")
]


class ScopeDescriptor(BaseModel):
    config: Dict[str, Any] = Field(default_factory=dict)
    lifecycle: Optional[Dict[str, Any]] = None

    model_config = ConfigDict(extra="forbid")


# ===== Service config (request payloads) =====

class ServiceConfig(BaseModel):
    name: str


class URLServiceConfig(ServiceConfig):
    url: str
    transport: Optional[str] = "streamable-http"
    headers: Optional[Dict[str, str]] = None


class CommandServiceConfig(ServiceConfig):
    command: str
    args: Optional[List[str]] = None
    env: Optional[Dict[str, str]] = None
    working_dir: Optional[str] = None


class MCPServerConfig(BaseModel):
    mcpServers: Dict[str, Dict[str, Any]]


ServiceConfigUnion = Union[
    URLServiceConfig, CommandServiceConfig, MCPServerConfig, Dict[str, Any]
]
