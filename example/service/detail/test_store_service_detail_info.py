"""
测试：Store 获取服务信息
功能：测试使用 service_info() 获取服务的详细配置信息
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
print("测试：Store 获取服务信息")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加服务
print("\n1️⃣ 初始化 Store 并添加服务")
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务 'weather' 已添加并就绪")

# 2️⃣ 获取服务的 ServiceProxy
print("\n2️⃣ 获取服务的 ServiceProxy")
service_proxy = store.for_store().find_service("weather")
print(f"✅ ServiceProxy 获取成功")

# 3️⃣ 使用 service_info() 获取服务信息
print("\n3️⃣ 使用 service_info() 获取服务信息")
info = service_proxy.service_info()
print(f"✅ 服务信息获取成功")
print(f"   返回类型: {type(info)}")

# 4️⃣ 展示服务信息的主要字段
print("\n4️⃣ 展示服务信息的主要字段")
print(f"📋 基本信息:")
if 'name' in info:
    print(f"   服务名称: {info['name']}")
if 'type' in info:
    print(f"   服务类型: {info['type']}")
if 'config' in info:
    print(f"   配置信息: {info['config']}")

# 5️⃣ 展示完整的服务信息（JSON 格式）
print("\n5️⃣ 完整的服务信息（JSON 格式）:")
print("-" * 60)
print(json.dumps(info, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 6️⃣ 检查常见字段
print("\n6️⃣ 检查服务信息中的常见字段")
common_fields = ['name', 'type', 'config', 'state', 'created_at', 'updated_at']
for field in common_fields:
    if field in info:
        print(f"   ✅ {field}: {info[field]}")
    else:
        print(f"   ⚠️ {field}: 未找到")

# 7️⃣ 添加另一个本地服务并对比信息
print("\n7️⃣ 添加本地服务并对比信息")
local_service = {
    "mcpServers": {
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
}
store.for_store().add_service(local_service)
store.for_store().wait_service("howtocook", timeout=30.0)
print(f"✅ 本地服务 'howtocook' 已添加")

local_proxy = store.for_store().find_service("howtocook")
local_info = local_proxy.service_info()
print(f"\n📋 本地服务信息:")
print(f"   服务名称: {local_info.get('name', 'N/A')}")
print(f"   服务类型: {local_info.get('type', 'N/A')}")
print(f"   配置: {local_info.get('config', 'N/A')}")

print("\n💡 service_info() 特点:")
print("   - 返回服务的详细配置信息")
print("   - 包含服务名称、类型、配置等")
print("   - 可能包含创建/更新时间")
print("   - 适合查看服务的完整配置")
print("   - 不同类型服务的 config 字段不同")

print("\n💡 使用场景:")
print("   - 调试服务配置")
print("   - 查看服务类型（URL/命令/市场）")
print("   - 导出服务配置")
print("   - 对比不同服务的配置")

print("\n" + "=" * 60)
print("✅ Store 获取服务信息测试完成")
print("=" * 60)

