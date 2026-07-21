# MCPStore 文档

MCPStore 用来**管理 MCP 服务**。

你可以在本地使用它，也可以连接远程运行的 MCPStore；需要时，还可以把它集成到 Rust 或 Python 项目中，或让编程助手等 MCP 客户端接入。

## 你想怎么使用？

<div class="grid cards" markdown>

-   :material-server-network:{ .lg .middle } __管理 MCP 服务__

    添加、配置、查看状态、检查健康、更新和删除 MCP 服务。

    [:octicons-arrow-right-24: 开始管理](management/overview.md)

-   :material-console:{ .lg .middle } __在本地使用__

    通过 CLI、TUI 或桌面端管理本机的 MCP 服务。

    [:octicons-arrow-right-24: 本地使用](clients/local.md)

-   :material-code-braces:{ .lg .middle } __集成到代码__

    在 Rust 或 Python 项目中调用 MCPStore 的能力。

    [:octicons-arrow-right-24: 代码集成](integration/overview.md)

-   :material-remote-desktop:{ .lg .middle } __连接远程实例__

    使用运行在服务器上的 MCPStore。

    [:octicons-arrow-right-24: 远程使用](remote/overview.md)

</div>

## 先理解三个概念

- [MCPStore 是什么](concepts/overview.md)
- [MCP 服务如何被管理](management/overview.md)
- [本地、远程与客户端的关系](concepts/connections.md)
