`python/src/mcpstore` 是当前项目的正式 Python 源码入口。

当前 Python 包源码目录是 `python/src/mcpstore`。

当前 Rust 绑定产物会打进：
- `python/src/mcpstore/_rust*`

## 角色组件（ControlPanel / DataPanel）

角色用代码组装，`setup_store` 不再携带 node_mode：

```python
from mcpstore import MCPStore, ControlPanel, DataPanel

store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))

# 决策节点：挂载自愈监督器（keep_alive 断线重连、健康状态机），幂等
ControlPanel(store).start()

# 执行节点：心跳 + 能力自报（node_status 行，updated_at 即存活信号）
data = DataPanel(store, node_id="edge-01", capabilities=["browser"])
await data.heartbeat()   # 15s 循环由宿主进程驱动
```
