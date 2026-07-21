# CLI 与 TUI

CLI 和 TUI 都用于管理 MCP 服务。它们使用同一份服务配置和运行状态。

## CLI

CLI 适合脚本、自动化和明确的单次操作。

```bash
mcpstore list --config-path ./mcp.json
mcpstore add wiki https://example.com/mcp --config-path ./mcp.json
```

完整命令见 [CLI 参考](../reference/cli.md)，按任务操作见[服务管理任务](../management/tasks.md)。

## TUI

TUI 适合人工查看服务状态、浏览服务详情和执行管理操作：

```bash
mcpstore tui --config-path ./mcp.json
```

如果使用独立的 TUI 二进制，也可以运行：

```bash
mcpstore-tui --config-path ./mcp.json
```

TUI 支持 `zh-cn` 和 `en-us`：

```bash
mcpstore tui --locale zh-cn --config-path ./mcp.json
```
