"""Rust-backed compatibility helpers for historical agent statistics."""

from __future__ import annotations

from mcpstore.core.store.rust_backend import _record_value


class AgentStatisticsMixin:
    """Compatibility mixin deriving agent summary from Rust-backed contexts."""

    def get_agents_summary(self):
        context = self._rust_context()
        agents = []
        total_services = 0
        total_tools = 0
        active_agents = 0
        for item in context.list_agents():
            agent_id = item.get("agent_id") or item.get("id")
            if not agent_id:
                continue
            stats = self._get_agent_statistics(str(agent_id))
            agents.append(stats)
            total_services += int(stats.get("service_count") or 0)
            total_tools += int(stats.get("tool_count") or 0)
            if stats.get("is_active"):
                active_agents += 1
        store_stats = context.get_stats()
        return _record_value({
            "total_agents": len(agents),
            "active_agents": active_agents,
            "total_services": total_services,
            "total_tools": total_tools,
            "store_services": store_stats.get("service_count", 0),
            "store_tools": store_stats.get("tool_count", 0),
            "agents": agents,
        })

    async def get_agents_summary_async(self):
        return self.get_agents_summary()

    def _get_agent_statistics(self, agent_id: str):
        return self._rust_context().find_agent(agent_id).get_stats()

    def _rust_context(self):
        return getattr(self, "_context", self)


__all__ = ["AgentStatisticsMixin"]
