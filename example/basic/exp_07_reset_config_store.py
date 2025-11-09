#!/usr/bin/env python3
"""
基础示例：重置配置 (Store 级别)
功能：演示如何重置配置，清除所有服务
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 添加一些测试服务
store.for_store().add_service({
    "mcpServers": {
        "mcpstore": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
})

# 重置前查看服务数
services_before = store.for_store().list_services()
print(f"重置前服务数: {len(services_before)}")

# 重置配置
store.for_store().reset_config()

# 重置后查看服务数
services_after = store.for_store().list_services()
print(f"重置后服务数: {len(services_after)}")

# 清空配置方便下次演示
store.for_store().reset_config()
