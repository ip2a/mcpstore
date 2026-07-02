"""Agent service name compatibility helpers."""

from __future__ import annotations

from typing import Any, Dict, List, Optional


class AgentServiceMapper:
    """Convert between agent-local service names and global service names."""

    separator = "_byagent_"

    def __init__(self, agent_id: str):
        self.agent_id = str(agent_id)
        self.suffix = f"{self.separator}{self.agent_id}"

    def to_global_name(self, local_name: str) -> str:
        value = str(local_name)
        if value.endswith(self.suffix):
            return value
        return f"{value}{self.suffix}"

    def to_local_name(self, global_name: str) -> str:
        value = str(global_name)
        if value.endswith(self.suffix):
            return value[: -len(self.suffix)]
        return value

    def is_agent_service(self, global_name: str) -> bool:
        return str(global_name).endswith(self.suffix)

    @classmethod
    def is_any_agent_service(cls, service_name: str) -> bool:
        return cls.separator in str(service_name)

    @classmethod
    def parse_agent_service_name(cls, global_name: str) -> tuple[str, str]:
        value = str(global_name)
        if cls.separator not in value:
            raise ValueError(f"Not an Agent service: {global_name}")
        local_name, agent_id = value.split(cls.separator, 1)
        if not local_name or not agent_id:
            raise ValueError(f"Invalid Agent service name format: {global_name}")
        return agent_id.strip(), local_name.strip()

    def filter_agent_services(self, global_services: Dict[str, Any]) -> Dict[str, Any]:
        return {
            self.to_local_name(name): config
            for name, config in global_services.items()
            if self.is_agent_service(name)
        }

    def convert_service_list_to_local(self, global_service_infos: List[Any]) -> List[Any]:
        local_service_infos = []
        for service_info in global_service_infos:
            name = getattr(service_info, "name", None)
            if name is None or not self.is_agent_service(name):
                continue
            local_name = self.to_local_name(name)
            if hasattr(service_info, "model_copy"):
                local_service_infos.append(service_info.model_copy(update={"name": local_name}))
            elif hasattr(service_info, "copy"):
                local_service_infos.append(service_info.copy(update={"name": local_name}))
            else:
                data = dict(vars(service_info))
                data["name"] = local_name
                local_service_infos.append(type(service_info)(**data))
        return local_service_infos

    def find_global_tool_name(
        self,
        local_tool_name: str,
        available_tools: List[str],
    ) -> Optional[str]:
        value = str(local_tool_name)
        if "_" not in value:
            return None
        local_service_name, tool_suffix = value.split("_", 1)
        global_service_name = self.to_global_name(local_service_name)
        expected = f"{global_service_name}_{tool_suffix}"
        if expected in available_tools:
            return expected
        prefix = f"{global_service_name}_"
        for tool_name in available_tools:
            if tool_name.startswith(prefix) and tool_name[len(prefix):] == tool_suffix:
                return tool_name
        return None

    def convert_config_to_local(self, global_config: Dict[str, Any]) -> Dict[str, Any]:
        servers = global_config.get("mcpServers") if isinstance(global_config, dict) else None
        if not isinstance(servers, dict):
            return dict(global_config)
        converted = dict(global_config)
        converted["mcpServers"] = self.filter_agent_services(servers)
        return converted

    def convert_config_to_global(self, local_config: Dict[str, Any]) -> Dict[str, Any]:
        servers = local_config.get("mcpServers") if isinstance(local_config, dict) else None
        if not isinstance(servers, dict):
            return dict(local_config)
        converted = dict(local_config)
        converted["mcpServers"] = {
            self.to_global_name(name): config
            for name, config in servers.items()
        }
        return converted
