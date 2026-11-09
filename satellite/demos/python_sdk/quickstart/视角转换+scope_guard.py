"""The current official API: cross-scope instances are rejected, and deletion by name only affects the current scope."""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig

URL = "http://115.190.220.103:2230/mcp"
SERVICE_NAME = "weather"


def add_service(context) -> str:
    context.add_service({"mcpServers": {SERVICE_NAME: {"url": URL}}})
    services = context.list_services()
    assert len(services) == 1, services
    return services[0]["instance_id"]


def main() -> None:
    store = MCPStore.setup_store(source=MemoryConfig(), debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("agent-a")
    for context in (store_context, agent_context):
        context.reset_config()
    try:
        store_instance_id = add_service(store_context)
        agent_instance_id = add_service(agent_context)
        assert store_instance_id != agent_instance_id

        try:
            agent_context.remove_service(instance_id=store_instance_id)
        except RuntimeError as error:
            assert "does not belong to scope" in str(error), error
            print("cross-scope rejection=", error)
        else:
            raise AssertionError("AgentContext accepted a Store instance_id")

        store_context.remove_service(service_name=SERVICE_NAME)
        assert store_context.list_services() == []
        services = agent_context.list_services()
        assert [service["instance_id"] for service in services] == [agent_instance_id]
        print("store services= []")
        print("agent services=", services)
    finally:
        for context in (store_context, agent_context):
            context.reset_config()


if __name__ == "__main__":
    main()
