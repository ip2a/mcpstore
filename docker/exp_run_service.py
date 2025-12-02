
#!/usr/bin/env python3
"""
运行服务示例
功能：启动MCPStore API服务器
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from mcpstore import MCPStore


def main():
    # 标题
    print("=" * 60)
    print("运行服务示例 - 启动MCPStore API服务器")
    print("=" * 60)

    # 初始化生产环境store
    print("[i] 初始化生产环境 Store...")
    prod_store = MCPStore.setup_store(
        debug=False,
        mcp_config_file=r'../test_workspaces/workspace1/mcp.json'
    )
    print("[✓] Store 初始化成功")

    # 启动API服务器
    print("[i] 启动 API 服务器...")
    print("[#] Host: 0.0.0.0")
    print("[#] Port: 18200")
    print("[i] 按 Ctrl+C 停止服务器")
    print("-" * 60)

    prod_store.start_api_server(
        host='0.0.0.0',
        port=18200,
        show_startup_info=False,
    )


if __name__ == "__main__":
    main()

