"""
Advanced direct-return example
Demonstrates advanced return-value handling for MCPStore tools
"""

from mcpstore import MCPStore


def main():
    """Demonstrate advanced return-value handling."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the sample service
    ctx.add_service({
        "name": "math_service",
        "url": "https://api.mathjs.org/v4/"
    })

    # Wait for the service to become ready
    ctx.wait_service("math_service")

    # Call the tool and handle its return value
    try:
        result = ctx.use_tool("evaluate", {
            "expression": "2 + 2 * 3"
        })
        # Process the returned result
        if result:
            processed_result = str(result).strip()
    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
