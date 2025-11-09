#!/usr/bin/env python3
"""
基础示例：重置配置 (Agent 级别)
功能：演示如何重置配置，清除 Agent 的所有服务
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 为Agent添加测试服务
store.for_agent("demo_agent").add_service({
    "mcpServers": {
        "mcpstore": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
})

# 重置前查看Agent服务数
services_before = store.for_agent("demo_agent").list_services()
print(f"重置前Agent服务数: {len(services_before)}")

# 重置配置（包括所有Agent的服务）
store.for_store().reset_config()

# 重置后查看Agent服务数
services_after = store.for_agent("demo_agent").list_services()
print(f"重置后Agent服务数: {len(services_after)}")

# 清空配置方便下次演示
store.for_store().reset_config()
