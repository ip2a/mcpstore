"""Role assembly example: control panel / data panel (per-panel placement config model).

Service + scope declarations live in the shared mcp.json::

    {
      "mcpServers": {
        "github": {
          "command": "npx",
          "args": ["-y", "@modelcontextprotocol/server-github"],
          "_mcpstore": {
            "scopes": {
              "store": {},
              "agents": {
                "agentA": { "config": { "env": { "GITHUB_TOKEN": "tok-a" } } }
              }
            }
          }
        }
      }
    }

Where each instance executes is the data panel's own decision, in its local
config.toml next to mcp.json — never in the shared config, never read by the
control panel::

    [placement]
    "agentA.github" = "delegate"   # proxy to the control panel
    "store.context7" = "delegate"

Keys pair scope and service ("{scope}.{service}"). Missing entries, an absent
section or unknown values mean local execution — pull the validated config from
the shared store and run it here (lazy connect on first call).

Control-panel node (cloud server)::

    from mcpstore import MCPStore, ControlPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=ControlPanel(),        # supervisor mounted at setup; connects every instance
    )

Data-panel node (resource-constrained edge device)::

    from mcpstore import MCPStore, DataPanel
    from mcpstore.config import RedisConfig

    store = MCPStore.setup_store(
        source=RedisConfig(url="redis://central:6379"),
        panel=DataPanel(panel_id="edge-01"),
    )

panel is a setup parameter; there is no second object — the default is ControlPanel.
"""

# This file is a documentation example; see the docstring for runtime imports.
