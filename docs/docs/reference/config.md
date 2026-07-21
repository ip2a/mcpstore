# 配置参考

MCPStore 的服务配置可以来自配置文件或数据库/键值存储。

## 常用选择

- `--config-path`：指定配置文件
- `--source local`：使用本地配置与键值存储，默认值
- `--source db`：只使用键值存储
- `--backend memory`：内存后端
- `--backend redis`：Redis 后端
- `--namespace`：指定键值存储命名空间

服务配置支持 URL 服务和 stdio 服务。添加服务时分别使用：

```bash
mcpstore add wiki https://example.com/mcp --config-path ./mcp.json
mcpstore add filesystem --config-path ./mcp.json -- npx -y @modelcontextprotocol/server-filesystem /tmp
```
