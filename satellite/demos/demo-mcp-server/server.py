"""Demo MCP server (official `mcp` SDK v2): tools + prompts + resources, remote via streamable HTTP."""

from mcp.server.mcpserver import MCPServer

mcp = MCPServer("demo-mcp-server")


# ---- Tools ----
@mcp.tool()
def add(a: int, b: int) -> int:
    """Add two numbers."""
    return a + b


@mcp.tool()
def echo(text: str) -> str:
    """Echo the given text back."""
    return f"echo: {text}"


# ---- Resources ----
@mcp.resource("demo://greeting")
def greeting() -> str:
    """A static greeting resource."""
    return "Hello from demo MCP server!"


@mcp.resource("demo://time")
def current_time() -> str:
    """Current server time."""
    from datetime import datetime, timezone

    return datetime.now(timezone.utc).isoformat()


# ---- Prompts ----
@mcp.prompt()
def review_code(code: str) -> str:
    """Prompt template for code review."""
    return f"Please review the following code and point out issues:\n\n{code}"


@mcp.prompt()
def translate(text: str, target_lang: str = "English") -> str:
    """Prompt template for translation."""
    return f"Translate the following text into {target_lang}:\n\n{text}"


if __name__ == "__main__":
    mcp.run(transport="streamable-http", host="0.0.0.0", port=8800)
