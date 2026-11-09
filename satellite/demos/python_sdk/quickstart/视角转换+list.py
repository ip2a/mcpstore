"""The current official API: identically named services create distinct instances in store and agent scopes."""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig

URL = "http://115.190.220.103:2230/mcp"
SERVICE_NAME = "weather"


def add_service(context) -> None:
    context.add_service({"mcpServers": {SERVICE_NAME: {"url": URL}}})


def one_service(context):
    services = context.list_services()
    assert len(services) == 1, services
    assert services[0]["service_name"] == SERVICE_NAME, services
    return services[0]


def main() -> None:
    store = MCPStore.setup_store(source=MemoryConfig(), debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("agent-a")
    for context in (store_context, agent_context):
        context.reset_config()
    try:
        add_service(store_context)
        add_service(agent_context)
        store_service = one_service(store_context)
        agent_service = one_service(agent_context)

        assert store_service["instance_id"] != agent_service["instance_id"]
        assert store_service["scope"] == {"type": "store"}
        assert agent_service["scope"] == {"type": "agent", "agent_id": "agent-a"}
        print("Store:", store_service)
        print("Agent(agent-a):", agent_service)
    finally:
        for context in (store_context, agent_context):
            context.reset_config()


if __name__ == "__main__":
    main()
