#!/usr/bin/env python3
"""
基础示例：更新服务 (Store 级别)
功能：演示如何更新服务配置
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

# 更新前查看配置
service_info_before = store.for_store().find_service("mcpstore").service_info()
print(f"更新前URL: {service_info_before.url}")

# 更新服务配置
store.for_store().find_service("mcpstore").update_config({
    "url": "https://mcp.context7.com/mcp"
})

# 更新后查看配置
config = store.config.load_config()
weather_config = config.get("mcpServers", {}).get("mcpstore", {})
print(f"更新后URL: {weather_config.get('url')}")

# 清空配置方便下次演示
store.for_store().reset_config()
