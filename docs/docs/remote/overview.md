# 连接远程 MCPStore

当 MCPStore 运行在服务器上时，本地客户端或其他设备可以连接这个实例，集中管理服务器上的 MCP 服务。

## 启动远程 API

在服务器上运行：

```bash
mcpstore api \
  --host 0.0.0.0 \
  --port 18200 \
  --allow-remote \
  --config-path ./mcp.json
```

API 默认只允许 loopback 地址。只有确认网络访问控制和认证配置后，才使用 `--allow-remote` 暴露到非本机地址。

## 让客户端接入

远程实例的地址、认证方式和客户端配置取决于接入端。先确认服务器的 API 地址可访问，再使用对应的 CLI、TUI、桌面端、Web 或代码入口连接。
