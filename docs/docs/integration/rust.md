# Rust 集成

Rust 是 MCPStore 核心能力的主要使用方式。适合在 Rust 服务或工具中直接使用 MCPStore crate。

Rust 集成的基本任务与 CLI 相同：

1. 创建或连接 MCPStore
2. 加载 MCP 服务配置
3. 添加、查询和控制服务
4. 发现或调用服务提供的能力

如果目标是启动独立的管理服务或 MCP Server，直接使用 Rust CLI 更简单：

```bash
mcpstore api --config-path ./mcp.json
mcpstore mcp-server --config-path ./mcp.json
```

- `api` 提供 HTTP 管理 API，默认监听 `127.0.0.1:18200`
- `mcp-server` 暴露 MCP Server，默认使用 `stdio`

Rust crate 的具体类型和函数应以当前公开 API 为准；用户流程与 [Python 集成](python.md)保持一致。
