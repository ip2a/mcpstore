#!/usr/bin/env python3
"""
基础示例：删除服务 (Store 级别)
功能：演示如何删除服务
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

# 初始化store
store = MCPStore.setup_store(debug=False)

# 添加服务
store.for_store().add_service({
    "mcpServers": {
        "mcpstore": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
})

# 等待服务就绪
store.for_store().wait_service("mcpstore")

# 删除前查看服务数
services_before = store.for_store().list_services()
print(f"删除前服务数: {len(services_before)}")

# 删除服务
store.for_store().find_service("mcpstore").delete_service()

# 删除后查看服务数
services_after = store.for_store().list_services()
print(f"删除后服务数: {len(services_after)}")

# 清空配置方便下次演示
store.for_store().reset_config()
