#!/usr/bin/env python3
"""
基础示例：调用远程工具 (Agent 级别)
功能：演示如何调用 Agent 的远程 HTTP MCP 服务工具
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 为Agent添加远程服务
store.for_agent("demo_agent").add_service({
    "mcpServers": {
        "mcpstore": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
})

# 等待服务就绪
store.for_agent("demo_agent").wait_service("mcpstore")

# 调用Agent远程工具
result = store.for_agent("demo_agent").use_tool('get_current_weather', {"query": "北京"})
print(result)

# 清空配置方便下次演示
store.for_store().reset_config()
