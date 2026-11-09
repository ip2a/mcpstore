"""The current official API: patches are located via the current scope, but they modify the shared service definition."""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig

URL = "http://115.190.220.103:2230/mcp"
SERVICE_NAME = "weather"


def service_config(context):
    services = context.list_services()
    assert len(services) == 1, services
    service = services[0]
    assert service["service_name"] == SERVICE_NAME, service
    return service["effective_config"]


def main() -> None:
    store = MCPStore.setup_store(source=MemoryConfig(), debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("agent-a")
    for context in (store_context, agent_context):
        context.reset_config()
    try:
        config = {"mcpServers": {SERVICE_NAME: {"url": URL}}}
        store_context.add_service(config)
        agent_context.add_service(config)
        agent_context.patch_service(
            service_name=SERVICE_NAME,
            updates={"headers": {"X-Agent": "agent-a"}},
        )
        agent_config = service_config(agent_context)
        store_config = service_config(store_context)

        assert agent_config["headers"] == {"X-Agent": "agent-a"}
        assert store_config["headers"] == {"X-Agent": "agent-a"}
        print("agent effective_config=", agent_config)
        print("store effective_config=", store_config)
    finally:
        for context in (store_context, agent_context):
            context.reset_config()


if __name__ == "__main__":
    main()
