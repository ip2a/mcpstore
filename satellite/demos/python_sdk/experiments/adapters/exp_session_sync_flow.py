"""
Synchronous session flow example
Demonstrates the MCPStore session context manager, which preserves state and cleans up automatically
"""

import time

from mcpstore import MCPStore

# Initialize MCPStore
store = MCPStore.setup_store(debug=False)
ctx = store.for_store()

# Add the Playwright browser service
demo_mcp = {
    "mcpServers": {
        "playwright": {
            "command": "npx",
            "args": ["@playwright/mcp"]
        }
    }
}

# Register the service and wait for it to become ready
ctx.add_service(demo_mcp)
ctx.wait_service("playwright")

# Use the session context manager
print("=== Synchronous session flow demo ===")

with ctx.with_session("browser_session") as session:
    print(f"Session created: {session.session_id}")

    # Bind the service to speed up the first call
    try:
        session.bind_service("playwright")
        print("Playwright service bound")
    except Exception as e:
        print(f"Service binding warning: {e}")

    # Tool call: navigate to the page
    t0 = time.perf_counter()
    result1 = session.use_tool(
        "playwright_browser_navigate",
        {"url": "https://www.baidu.com"},
        timeout=180
    )
    t1 = time.perf_counter()
    print(f"Page navigation finished in {t1 - t0:.3f}s")

    # Tool call: take a page snapshot to verify state persistence
    t2 = time.perf_counter()
    result2 = session.use_tool(
        "playwright_browser_snapshot",
        {"input": ""},
        timeout=120
    )
    t3 = time.perf_counter()
    print(f"Page snapshot finished in {t3 - t2:.3f}s")

    # Show a summary of the results
    print(f"Navigation result type: {type(result1).__name__}")
    print(f"Snapshot result preview: {str(result2)[:200]}...")

print("Session context exited (closed automatically)")

# Clean up the configuration
store.for_store().reset_config()

