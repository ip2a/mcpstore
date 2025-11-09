#!/usr/bin/env python3
"""
基础示例：列出工具 (Agent 级别)
功能：演示如何列出 Agent 的所有可用工具
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 为Agent添加服务
store.for_agent("demo_agent").add_service({
    "mcpServers": {
        "mcpstore": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
})

# 等待服务就绪
store.for_agent("demo_agent").wait_service("mcpstore")

# 打印Agent工具列表
tools = store.for_agent("demo_agent").list_tools()
print(tools)

# 清空配置方便下次演示
store.for_store().reset_config()
