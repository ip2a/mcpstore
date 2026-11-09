"""Show the configuration visible from each public perspective: Root, Store, Agent, and Session."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def service_config(name: str) -> dict:
    return {"mcpServers": {name: {"url": URL}}}


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("show-config-agent")
    store_context.reset_config()
    try:
        store_context.add_service(service_config("store-weather"))
        agent_context.add_service(service_config("agent-weather"))

        root_config = store.show_config()
        store_config = store_context.show_config()
        agent_config = agent_context.show_config()
        session = store.create_session(
            "show-config-session", scope="agent", agent_id="show-config-agent"
        )
        session_config = session.show_config()

        assert set(root_config["mcpServers"]) == {"store-weather", "agent-weather"}
        assert set(store_config["mcpServers"]) == {"store-weather"}
        assert set(agent_config["mcpServers"]) == {"agent-weather"}
        assert session_config == agent_config

        print("root config=", root_config)
        print("store config=", store_config)
        print("agent config=", agent_config)
        print("session config=", session_config)
    finally:
        store_context.reset_config()


if __name__ == "__main__":
    main()
