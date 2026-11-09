"""The current official API: scoped tools of identically named services are called within their own scope."""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig

URL = "http://115.190.220.103:2230/mcp"
SERVICE_NAME = "weather"


def add_ready_service(context) -> None:
    context.add_service({"mcpServers": {SERVICE_NAME: {"url": URL}}})
    context.wait_service(service_name=SERVICE_NAME, timeout=20.0)


def call_weather(context):
    tool = next(tool for tool in context.list_tools() if tool["tool_name"] == "get_weather")
    result = context.call_tool(tool["name"], {"city": "北京"})
    assert not result["is_error"], result
    assert result["content"], result
    return tool, result


def main() -> None:
    store = MCPStore.setup_store(source=MemoryConfig(), debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("agent-a")
    for context in (store_context, agent_context):
        context.reset_config()
    try:
        add_ready_service(store_context)
        add_ready_service(agent_context)
        store_tool, store_result = call_weather(store_context)
        agent_tool, agent_result = call_weather(agent_context)

        assert store_tool["scope"] == {"type": "store"}
        assert agent_tool["scope"] == {"type": "agent", "agent_id": "agent-a"}
        print(f"Store tool={store_tool['name']} content={store_result['content']}")
        print(f"Agent(agent-a) tool={agent_tool['name']} content={agent_result['content']}")
    finally:
        for context in (store_context, agent_context):
            context.reset_config()


if __name__ == "__main__":
    main()
