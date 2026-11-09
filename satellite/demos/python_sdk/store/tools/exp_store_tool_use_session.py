"""
MCPStore session-based tool call example (Store scope)
Demonstrates calling tools in session mode, where state persists across calls.
Supports chained tool calls and state management for complex workflows.
"""

from mcpstore import MCPStore
import time

# Initialize the Store and add the Playwright service
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "playwright": {
            "command": "npx",
            "args": ["@playwright/mcp"]
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("playwright", timeout=30.0)

# List the available tools
tools = store.for_store().list_tools()
tool_names = [t.name for t in tools]

# Create a session and bind the service
session = store.for_store().create_session("playwright_test_session")

try:
    session.bind_service("playwright")
except Exception as e:
    pass

# First tool call - navigate to Baidu
start_time = time.perf_counter()
try:
    result1 = session.use_tool(
        "playwright_browser_navigate",
        {"url": "https://www.baidu.com"},
        timeout=180
    )
    end_time = time.perf_counter()
    elapsed_time1 = end_time - start_time
except Exception as e:
    pass

# Wait for the page to load
time.sleep(3)

# Second tool call - capture a page snapshot (tests state persistence)
start_time = time.perf_counter()
try:
    result2 = session.use_tool(
        "playwright_browser_snapshot",
        {"input": ""},
        timeout=180
    )
    end_time = time.perf_counter()
    elapsed_time2 = end_time - start_time

    # Verify state persistence
    result2_str = str(result2)
    state_preserved = "baidu.com" in result2_str
except Exception as e:
    pass

# Retrieve session info
session_id = session.session_id

# Clear the configuration so the demo can run again cleanly
store.for_store().reset_config()

