#!/usr/bin/env python3
"""
基础示例：添加本地服务 (Agent 级别)
功能：演示如何为 Agent 添加一个本地 MCP 服务
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 为Agent添加本地服务
store.for_agent("demo_agent").add_service({
    "mcpServers": {
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
})

# 等待服务就绪
store.for_agent("demo_agent").wait_service("howtocook")

# 打印Agent服务列表
services = store.for_agent("demo_agent").list_services()
print(services)

# 清空配置方便下次演示
store.for_store().reset_config()
