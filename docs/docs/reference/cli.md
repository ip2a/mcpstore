# CLI 参考

运行 `mcpstore --help` 查看全部命令，运行 `mcpstore <命令> --help` 查看单个命令参数。

## 服务管理

| 命令 | 用途 |
| --- | --- |
| `add` | 添加服务 |
| `add-json` | 使用 JSON 添加服务 |
| `list` | 列出服务 |
| `get` | 查看服务详情 |
| `update` | 更新服务配置 |
| `remove` | 删除服务 |
| `connect` | 连接服务 |
| `disconnect` | 断开服务 |
| `restart` | 重启服务 |
| `check` | 健康检查 |
| `wait` | 等待服务就绪 |
| `tools` | 查看服务工具 |
| `call` | 调用工具 |
| `resource` | 读取 Resource |
| `prompt` | 获取 Prompt |

## 运行入口

| 命令 | 用途 |
| --- | --- |
| `tui` | 启动 TUI |
| `web` | 启动 Web 管理界面 |
| `api` | 启动 HTTP API |
| `mcp-server` | 暴露 MCP Server |
| `start` / `stop` | 管理后台运行实例 |

## 公共存储参数

服务管理命令通常支持：

```text
--config-path PATH
--source local|db
--backend memory|redis|openkeyv_memory|openkeyv_redis
--redis-url URL
--namespace NAME
```

完整参数以当前二进制的 `--help` 输出为准。
