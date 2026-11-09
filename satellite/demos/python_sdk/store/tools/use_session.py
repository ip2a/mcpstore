"""
Test: Store tool calls - the with-session pattern
Purpose: exercise the with context manager to manage a session with automatic resource cleanup
Context: Store level
"""

# MCPStore example import setup
import sys
sys.path.insert(0, "../..")
from example_utils import setup_example_import
setup_example_import()  # Set up the import path automatically



from mcpstore import MCPStore
import time

print("=" * 60)
print("Test: Store tool calls - the with-session pattern")
print("=" * 60)

# 1️⃣ Initialize the Store and add the Playwright service
print("\n1️⃣ Initialize the Store and add the Playwright service")
store = MCPStore.setup_store(debug=False)
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
print(f" Service 'playwright' added and ready")

# 2️⃣ Create a session context with a with statement
print("\n2️⃣ Create a session context with a with statement")
print(f"   Session ID: browser_session")
print(f"   Feature: resources are managed automatically and cleaned up on exit")

# 3️⃣ Run operations inside the session context
print("\n3️⃣ Run operations inside the session context")
with store.for_store().with_session("browser_session") as session:
    print(f" Session context created: {session.session_id}")
    
    # 4️⃣ Bind the service
    print("\n4️⃣ Bind the service to the session")
    session.bind_service("playwright")
    print(f" Service bound")
    
    # 5️⃣ First tool call - navigate to Baidu
    print("\n5️⃣ First tool call - navigate to Baidu")
    print(f"   Tool: playwright_browser_navigate")
    print(f"   Args: url='https://www.baidu.com'")
    
    try:
        result1 = store.for_store().use_tool(
            "playwright_browser_navigate",
            {"url": "https://www.baidu.com"},
            timeout=180
        )
        print(f" Navigation succeeded")
        result_str = str(result1)
        if len(result_str) > 150:
            print(f"   Result: {result_str[:150]}...")
        else:
            print(f"   Result: {result_str}")
    except Exception as e:
        print(f"❌ Navigation failed: {e}")
    
    # 6️⃣ Wait for the page to load
    print("\n6️⃣ Wait for the page to load")
    print(f"   Waiting 5 seconds...")
    time.sleep(5)
    
    # 7️⃣ Second tool call - navigate to the MCPStore website
    print("\n7️⃣ Second tool call - navigate to the MCPStore website")
    print(f"   Tool: playwright_browser_navigate")
    print(f"   Args: url='https://www.mcpstore.wiki'")
    
    try:
        result2 = store.for_store().use_tool(
            "playwright_browser_navigate",
            {"url": "https://www.mcpstore.wiki"},
            timeout=120
        )
        print(f" Navigation succeeded")
        result_str = str(result2)
        if len(result_str) > 150:
            print(f"   Result: {result_str[:150]}...")
        else:
            print(f"   Result: {result_str}")
    except Exception as e:
        print(f"❌ Navigation failed: {e}")
    
    # 8️⃣ Wait again
    print("\n8️⃣ Wait for the page to load")
    print(f"   Waiting 5 seconds...")
    time.sleep(5)
    
    print("\n Session operations complete, exiting the context")


print("\n" + "=" * 60)
print(" Store tool calls - the with-session pattern test complete")
print("=" * 60)

