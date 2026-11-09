"""
Direct-return test example
"""

from mcpstore import MCPStore


def main():
    """Demonstrate the core feature."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the sample service
    ctx.add_service({service_config})

    # Wait for the service to become ready
    ctx.wait_service("test_service")

    # Core feature demo
    try:
        # Perform the operation for this file type
        # Test the direct-return feature
        ctx.use_tool("test_tool", {"param": "value"})
    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
