# 快速开始

本页用 CLI 在本地启动并管理一个 MCP 服务。

## 1. 准备配置文件

MCPStore 默认使用本地配置源。也可以通过 `--config-path` 指定配置文件：

```bash
mcpstore list --config-path ./mcp.json
```

## 2. 添加服务

### 添加远程 HTTP 服务

```bash
mcpstore add mcpstore-wiki https://example.com/mcp \
  --config-path ./mcp.json
```

### 添加 stdio 服务

`--` 后面的内容会作为服务命令及其参数：

```bash
mcpstore add filesystem --config-path ./mcp.json -- npx -y @modelcontextprotocol/server-filesystem /tmp
```

如果命令参数可能与 MCPStore 参数冲突，使用 `--` 分隔。

## 3. 查看服务

```bash
mcpstore list --config-path ./mcp.json
mcpstore get filesystem --config-path ./mcp.json
```

## 4. 检查和控制服务

```bash
mcpstore check filesystem --config-path ./mcp.json
mcpstore restart filesystem --config-path ./mcp.json
mcpstore remove filesystem --config-path ./mcp.json
```

## 其他入口

- [CLI 与 TUI](../clients/local.md)
- [桌面端与 Web](../clients/desktop.md)
- [集成到代码](../integration/overview.md)
