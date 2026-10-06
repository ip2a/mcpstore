"""Role assembly example: control panel / data panel (service-level placement execution model).

Service-level placement (mcp.json)::

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

Services absent from placement are executed by the control panel by default.

Control-panel node (cloud server)::

    from mcpstore import MCPStore, ControlPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=ControlPanel(),        # supervisor mounted at setup
    )

Data-panel node (resource-constrained edge device)::

    from mcpstore import MCPStore, DataPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=DataPanel(panel_id="edge-01"),  # shared-store mode: no execution; writes only emit events
    )

panel is a setup parameter; there is no second object — the default is ControlPanel.
"""

# This file is a documentation example; see the docstring for runtime imports.
