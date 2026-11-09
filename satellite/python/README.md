`python/src/mcpstore` 是当前项目的正式 Python 源码入口。

当前 Python 包源码目录是 `python/src/mcpstore`。

当前 Rust 绑定产物会打进：
- `python/src/mcpstore/_rust*`

## 角色组件（ControlPanel / DataPanel）

角色用代码组装，`setup_store` 不再携带 node_mode：

```python
from mcpstore import MCPStore, ControlPanel, DataPanel

store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))

# 控制面板（缺省）：setup 即挂载自愈监督器（keep_alive 断线重连、健康状态机）
store = MCPStore.setup_store(source=redis_cfg, panel=ControlPanel())

# 数据面板：load 时自动拉取 placement 命中的服务本地建连（无周期任务）
store = MCPStore.setup_store(source=redis_cfg, panel=DataPanel(panel_id="edge-01"))
```
