# demo-mcp-server

官方 `mcp` SDK (v2) 构建的测试用 MCP server：tools + resources + prompts，streamable HTTP 远程模式。

```bash
python3 -m venv .venv && .venv/bin/pip install "mcp[cli]"
.venv/bin/python server.py   # http://0.0.0.0:8800/mcp
```

注意：v2 里 FastMCP 已改名 `MCPServer`（`from mcp.server.mcpserver import MCPServer`），streamable http 客户端入口是 `streamable_http_client`（yield 2 个值）。
