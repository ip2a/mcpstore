"""角色组装示例：控制面板 / 数据面板。

云端决策节点（资源充足）::

    from mcpstore import MCPStore, ControlPanel

    store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))
    ControlPanel(store).start()  # 挂载自愈监督器，常驻进程保持运行

边缘执行节点（资源受限）::

    import asyncio
    from mcpstore import MCPStore, DataPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))
    panel = DataPanel(store, node_id="edge-01", capabilities=["browser"])
    asyncio.run(panel.heartbeat())  # 单次心跳（可选验证）
    panel_loop = ...               # 15s 循环由宿主进程 spawn/abort 控制
"""

# 此文件为文档示例；运行时导入见 docstring。
