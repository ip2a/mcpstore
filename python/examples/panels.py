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
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=ControlPanel(),        # setup 即挂载自愈监督器
    )

数据面板节点（资源受限终端）::

    from mcpstore import MCPStore, DataPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=DataPanel(panel_id="edge-01"),  # 共享库模式不执行，写操作只进事件
    )

panel 是 setup 的参数，不存在第二个对象；缺省即 ControlPanel。
"""

# 此文件为文档示例；运行时导入见 docstring。
