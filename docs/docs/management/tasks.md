# 服务管理任务

## 添加服务

使用 `mcpstore add` 添加 URL 服务或 stdio 服务；使用 `mcpstore add-json` 直接传入 JSON 配置。

```bash
mcpstore add wiki https://example.com/mcp --config-path ./mcp.json
mcpstore add local-tool --config-path ./mcp.json -- uvx my-mcp-tool
```

HTTP 请求头和 stdio 环境变量可以重复传入：

```bash
mcpstore add private-api https://example.com/mcp \
  --header Authorization='Bearer TOKEN' \
  --config-path ./mcp.json

mcpstore add local-tool --config-path ./mcp.json --env API_KEY=value -- uvx my-mcp-tool
```

## 查看服务

```bash
mcpstore list --config-path ./mcp.json
mcpstore get wiki --config-path ./mcp.json
```

## 连接与断开

```bash
mcpstore connect wiki --config-path ./mcp.json
mcpstore disconnect wiki --config-path ./mcp.json
```

## 检查、等待和重启

```bash
mcpstore check wiki --config-path ./mcp.json
mcpstore wait wiki --config-path ./mcp.json
mcpstore restart wiki --config-path ./mcp.json
```

## 更新和删除

```bash
mcpstore update wiki --config-path ./mcp.json
mcpstore remove wiki --config-path ./mcp.json
```

更新命令的具体参数以 `mcpstore update --help` 为准；删除前确认服务名称和配置源正确。
