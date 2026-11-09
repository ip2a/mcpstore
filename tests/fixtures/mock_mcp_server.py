from mcp.server.fastmcp import FastMCP


mcp = FastMCP("mcpstore-cli-fixture")


@mcp.tool()
def greet(name: str) -> str:
    """Greet caller."""
    return f"Hello, {name}!"


@mcp.resource("fixture://docs/readme")
def readme() -> str:
    """Fixture README resource."""
    return "This is the MCPStore fixture resource."


@mcp.prompt()
def explain(topic: str = "overview") -> str:
    """Fixture prompt."""
    return f"Explain {topic} via fixture prompt."


if __name__ == "__main__":
    mcp.run("stdio")
