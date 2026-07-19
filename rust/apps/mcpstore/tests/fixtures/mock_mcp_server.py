import os

from mcp.server.fastmcp import FastMCP


mcp = FastMCP("mcpstore-cli-fixture")
label = os.getenv("MCPSTORE_FIXTURE_LABEL")


def labeled(default: str) -> str:
    return f"{label}: {default}" if label else default


@mcp.tool()
def greet(name: str) -> str:
    """Greet caller."""
    return labeled(f"Hello, {name}!")


@mcp.resource("fixture://docs/readme")
def readme() -> str:
    """Fixture README resource."""
    return labeled("This is the MCPStore fixture resource.")


if os.getenv("MCPSTORE_FIXTURE_TEMPLATE"):

    @mcp.resource("fixture://docs/{name}")
    def document(name: str) -> str:
        """Fixture resource template."""
        return labeled(f"Fixture document: {name}")


@mcp.prompt()
def explain(topic: str = "overview") -> str:
    """Fixture prompt."""
    return labeled(f"Explain {topic} via fixture prompt.")


if __name__ == "__main__":
    mcp.run("stdio")
