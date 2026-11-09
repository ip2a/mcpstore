# mcpstore_mcpservice（参考快照，未实现）

这是一个**非编译的参考快照**，不是可运行的项目。

## 来源

这些 `.rs` 文件原先是 `mcpstore` 核心 lib（`rust/crates/mcpstore/src/mcp_server/`）里的
MCP server 实现代码。在「把 MCP server 能力移出 core」的重构中：

- **活代码**已搬到 CLI app：`rust/apps/mcpstore/src/mcp_server/`（`mcpstore mcp-server`
  子命令由它驱动，经 npm / curl 交付）。
- core lib 与 Python 绑定**不再包含**任何 MCP server 能力，也不再依赖 axum。
- 这里的副本是搬移前的**原始形态**，保留作参考。

## 为什么放这里

未来若要做一个**独立的、基于 mcpstore 的 MCP 服务项目**（例如通过 `mcpstore api` HTTP
接口实现的 MCP server，或直接 `cargo add mcpstore` 用 lib 实现），可以以这份代码为起点。

## 注意

- 文件里的 `crate::xxx` 路径指向**旧的 core 上下文**，脱离原 crate 后**无法直接编译**。
- 想要当前可工作的实现，看 `rust/apps/mcpstore/src/mcp_server/`，不要改这里。
