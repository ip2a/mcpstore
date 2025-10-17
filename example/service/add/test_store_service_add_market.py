"""
测试：Store 通过 mcpServers 批量添加服务（替代市场安装示例）
功能：测试一次性添加多个服务，并使用 wait_service 等待状态
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：Store 通过 mcpServers 批量添加服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 准备多服务配置（mcpServers）
print("\n2️⃣ 准备多服务配置（mcpServers）")
services_config = {
    "mcpServers": {
        "mcpstore-wiki": {
            "url": "https://mcpstore.wiki/mcp"
        },
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
}
print("📋 配置内容:\n" + json.dumps(services_config, indent=2, ensure_ascii=False))

# 3️⃣ 添加服务
print("\n3️⃣ 添加服务")
store.for_store().add_service(services_config)
print(f"✅ 服务批量添加已触发（不等待）")

# 4️⃣ 等待服务就绪（可选）
print("\n4️⃣ 等待服务就绪（可选）")
store.for_store().wait_services(["mcpstore-wiki", "howtocook"], status="healthy", timeout=60.0)
print(f"✅ 等待完成")

# 5️⃣ 验证服务已添加
print("\n5️⃣ 验证服务已添加")
services = store.for_store().list_services()
print(f"✅ 当前服务数量: {len(services)}")
for svc in services:
    print(f"   - {svc.name}")

# 6️⃣ 列出服务的工具（如有）
print("\n6️⃣ 列出服务的工具")
tools = store.for_store().list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

print("\n" + "=" * 60)
print("✅ Store 批量添加服务测试完成")
print("=" * 60)

