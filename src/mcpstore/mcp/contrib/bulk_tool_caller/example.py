"""Sample code for MCPStore using MCPMixin."""

from mcpstore.mcp import MCPStore
from mcpstore.mcp.contrib.bulk_tool_caller import BulkToolCaller

mcp = MCPStore()


@mcp.tool
def echo_tool(text: str) -> str:
    """Echo the input text"""
    return text


bulk_tool_caller = BulkToolCaller()

bulk_tool_caller.register_tools(mcp)
