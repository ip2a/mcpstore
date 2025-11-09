#!/usr/bin/env python3
"""
基础示例：删除服务 (Agent 级别)
功能：演示如何删除 Agent 的服务
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

# 删除前查看Agent服务数
services_before = store.for_agent("demo_agent").list_services()
print(f"删除前Agent服务数: {len(services_before)}")

# 删除Agent服务
store.for_agent("demo_agent").find_service("mcpstore").delete_service()

# 删除后查看Agent服务数
services_after = store.for_agent("demo_agent").list_services()
print(f"删除后Agent服务数: {len(services_after)}")

# 清空配置方便下次演示
store.for_store().reset_config()
