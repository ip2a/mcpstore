"""Sample code for MCPKit using MCPMixin."""

from mcpstore.mcp import MCPKit
from mcpstore.mcp.contrib.bulk_tool_caller import BulkToolCaller

mcp = MCPKit()


@mcp.tool
def echo_tool(text: str) -> str:
    """Echo the input text"""
    return text


bulk_tool_caller = BulkToolCaller()

bulk_tool_caller.register_tools(mcp)
