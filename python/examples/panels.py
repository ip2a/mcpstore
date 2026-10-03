"""角色组装示例：控制面板 / 数据面板（服务级 placement 执行模型）。

服务级 placement（mcp.json）::

    {
      "mcpServers": {
        "filesystem": {
          "command": "npx",
          "args": ["-y", "server-filesystem", "/workspace"],
          "_mcpstore": {
            "scopes": { "store": {} },
            "placement": {
              "edge-01": { "args": ["-y", "server-filesystem", "/home/user"] },
              "edge-02": { "args": ["-y", "server-filesystem", "/data"] }
            }
          }
        }
      }
    }

未出现在 placement 里的服务默认由控制面板执行。

控制面板节点（云端服务器）::

    from mcpstore import MCPStore, ControlPanel

    store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))
    ControlPanel(store).start()   # 挂载自愈监督器；进程常驻

数据面板节点（资源受限终端）::

    import asyncio
    from mcpstore import MCPStore, DataPanel

    store = MCPStore.setup_store(source=RedisConfig(url="redis://central:6379"))
    panel = DataPanel(store, panel_id="edge-01")

    async def main():
        connected = await panel.serve()   # 拉取 placement 命中的服务，本地建连
        print(f"serving {connected} service(s)")
        # 面板零周期任务：只在 serve/call 等实际交互时活动

    asyncio.run(main())
"""

# 此文件为文档示例；运行时导入见 docstring。
