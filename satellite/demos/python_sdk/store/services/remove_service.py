"""Official API: remove_service removes a service only from the current Store / Agent scope."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def add_ready_service(context, service_name: str):
    context = context.add_service({"mcpServers": {service_name: {"url": URL}}})
    return context.wait_service(service_name=service_name, timeout=20.0).find_service(
        service_name=service_name
    )


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("agent-a")
    for context in (store_context, agent_context):
        context.reset_config()
    try:
        store_service = add_ready_service(store_context, "weather")
        agent_service = add_ready_service(agent_context, "weather")
        assert agent_service.remove_service() is True
        assert agent_context.list_services() == []
        assert store_context.find_service(service_name="weather").info()["instance_id"] == (
            store_service.info()["instance_id"]
        )

        assert store_service.remove_service() is True
        assert store_context.list_services() == []
        print("removed the same name from independent Store and Agent scopes")
    finally:
        for context in (store_context, agent_context):
            context.reset_config()


if __name__ == "__main__":
    main()
