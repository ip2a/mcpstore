#!/usr/bin/env python3
"""
Final cleanup and refactoring script
Creates simplified versions for the remaining files
"""

import os
from pathlib import Path

# Base simplification template
BASE_TEMPLATE = '''"""
{description}
"""

from mcpstore import MCPStore


def main():
    """Demonstrate the core feature."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the sample service
    ctx.add_service({{service_config}})

    # Wait for the service to become ready
    ctx.wait_service("{service_name}")

    # Core feature demo
    try:
        # Perform the operation for this file type
        {core_operations}
    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
'''

# Configuration of files to simplify
FILES_TO_REFACTOR = {
    "exp_return_direct_test.py": {
        "description": "Direct-return test example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Test the direct-return feature\nctx.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_comparison.py": {
        "description": "Session comparison example",
        "service_config": '"name": "playwright", "command": "npx", "args": ["-y", "@playwright/mcp@latest", "server"]',
        "service_name": "playwright",
        "core_operations": "# Compare the different session modes\nctx.session_auto(\"test_session\")\nsession = ctx.current_session()\nsession.use_tool(\"playwright_browser_navigate\", {\"url\": \"https://www.baidu.com\"})\nctx.session_manual()"
    },

    "exp_session_fix_verification.py": {
        "description": "Session fix verification example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Verify the session fix\nsession = ctx.create_session(\"fix_test\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_persistence_fix.py": {
        "description": "Session persistence fix example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Demonstrate session persistence\nsession = ctx.create_session(\"persistent_session\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_sync_click.py": {
        "description": "Session-synchronized click example",
        "service_config": '"name": "playwright", "command": "npx", "args": ["-y", "@playwright/mcp@latest", "server"]',
        "service_name": "playwright",
        "core_operations": "# Demonstrate session-synchronized clicks\nctx.session_auto(\"sync_session\")\nsession = ctx.current_session()\nsession.use_tool(\"playwright_browser_navigate\", {\"url\": \"https://www.baidu.com\"})\nsession.use_tool(\"playwright_browser_click\", {\"selector\": \"#kw\"})"
    },

    "exp_special_playwright_session.py": {
        "description": "Special Playwright session example",
        "service_config": '"name": "playwright", "command": "npx", "args": ["-y", "@playwright/mcp@latest", "server"]',
        "service_name": "playwright",
        "core_operations": "# Special session demo\nsession = ctx.create_session(\"special_playwright\")\nsession.bind_service(\"playwright\")\nsession.use_tool(\"playwright_browser_navigate\", {\"url\": \"https://www.baidu.com\"})"
    },

    "exp_store_langchain_agent_session.py": {
        "description": "Store LangChain agent session example",
        "service_config": '"name": "playwright", "command": "npx", "args": ["-y", "@playwright/mcp@latest", "server"]',
        "service_name": "playwright",
        "core_operations": "# LangChain agent session demo\ncat ctx.session_auto(\"agent_session\")\nlc_adapter = ctx.for_langchain()\ntools = lc_adapter.list_tools()"
    },

    "exp_session_basics.py": {
        "description": "Session basics example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Basic session operations\nsession = ctx.create_session(\"basic_session\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_context_manager.py": {
        "description": "Session context manager example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Context manager demo\nsession = ctx.create_session(\"context_manager\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_direct_id.py": {
        "description": "Direct session ID example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Direct session ID demo\nsession = ctx.create_session(\"direct_id_session\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_session_persistent_client.py": {
        "description": "Persistent session client example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Persistent client demo\nsession = ctx.create_session(\"persistent_client\")\nsession.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_sync_flow.py": {
        "description": "Synchronous flow example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Synchronous flow demo\nctx.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_tool_calling_agent.py": {
        "description": "Tool-calling agent example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Tool-calling agent demo\nctx.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_scenario_basic.py": {
        "description": "Basic scenario example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Basic scenario demo\nctx.use_tool(\"test_tool\", {\"param\": \"value\"})"
    },

    "exp_tool_conversion_basic.py": {
        "description": "Basic tool conversion example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# Tool conversion demo\nopenai_tools = ctx.for_openai().list_tools()\nlangchain_tools = ctx.for_langchain().list_tools()"
    },

    "exp_simple_openai_example.py": {
        "description": "Simple OpenAI example",
        "service_config": '"name": "test_service", "url": "https://api.example.com/mcp"',
        "service_name": "test_service",
        "core_operations": "# OpenAI adapter demo\nopenai_adapter = ctx.for_openai()\ntools = openai_adapter.list_tools()"
    }
}

def main():
    """Run the final refactoring."""

    current_dir = Path(".")
    refactored_count = 0

    for filename, config in FILES_TO_REFACTOR.items():
        filepath = current_dir / filename

        if filepath.exists():
            content = BASE_TEMPLATE.format(
                description=config["description"],
                service_config=config["service_config"],
                service_name=config["service_name"],
                core_operations=config["core_operations"]
            )

            filepath.write_text(content, encoding='utf-8')
            print(f"Refactored: {filename}")
            refactored_count += 1
        else:
            print(f"File not found, skipping: {filename}")

    print(f"\\nRefactoring complete! Processed {refactored_count} files.")

    # Delete the temporary script
    batch_script = current_dir / "batch_refactor_remaining.py"
    if batch_script.exists():
        batch_script.unlink()
        print("Temporary script removed")

if __name__ == "__main__":
    main()