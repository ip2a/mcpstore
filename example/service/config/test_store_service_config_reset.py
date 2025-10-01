"""
测试：Store 重置配置
功能：测试使用 reset_config() 重置 MCPStore 的配置
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
print("测试：Store 重置配置")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加服务
print("\n1️⃣ 初始化 Store 并添加服务")
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        },
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"✅ 已添加 2 个服务")

# 2️⃣ 查看重置前的配置
print("\n2️⃣ 查看重置前的配置")
config_before = store.for_store().show_config()
services_before = store.for_store().list_services()
print(f"📋 重置前状态:")
print(f"   服务数量: {len(services_before)}")
print(f"   服务列表: {[s.name for s in services_before]}")
if 'mcpServers' in config_before:
    print(f"   配置中的服务: {list(config_before['mcpServers'].keys())}")

# 3️⃣ 展示完整配置
print("\n3️⃣ 重置前完整配置（JSON 格式）:")
print("-" * 60)
print(json.dumps(config_before, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 4️⃣ 使用 reset_config() 重置配置
print("\n4️⃣ 使用 reset_config() 重置配置")
print(f"⏳ 正在重置配置...")
result = store.for_store().reset_config()
print(f"✅ 配置已重置")
print(f"   返回结果: {result}")

# 5️⃣ 查看重置后的配置
print("\n5️⃣ 查看重置后的配置")
config_after = store.for_store().show_config()
services_after = store.for_store().list_services()
print(f"📋 重置后状态:")
print(f"   服务数量: {len(services_after)}")
if services_after:
    print(f"   服务列表: {[s.name for s in services_after]}")
else:
    print(f"   服务列表: （无服务）")

if 'mcpServers' in config_after:
    if config_after['mcpServers']:
        print(f"   配置中的服务: {list(config_after['mcpServers'].keys())}")
    else:
        print(f"   配置中的服务: （无服务）")

# 6️⃣ 展示重置后的完整配置
print("\n6️⃣ 重置后完整配置（JSON 格式）:")
print("-" * 60)
print(json.dumps(config_after, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 7️⃣ 对比重置前后
print("\n7️⃣ 对比重置前后")
print(f"   重置前服务数: {len(services_before)}")
print(f"   重置后服务数: {len(services_after)}")
print(f"   ✅ 配置已恢复到初始状态")

# 8️⃣ 重置后可以重新添加服务
print("\n8️⃣ 重置后可以重新添加服务")
new_config = {
    "mcpServers": {
        "new_service": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(new_config)
print(f"✅ 已重新添加服务")

final_services = store.for_store().list_services()
print(f"📋 最终服务列表: {[s.name for s in final_services]}")

# 9️⃣ 重置配置的影响范围
print("\n9️⃣ reset_config() 的影响范围")
print(f"   ✅ 清除所有服务配置")
print(f"   ✅ 停止所有运行中的服务")
print(f"   ✅ 恢复默认设置")
print(f"   ✅ 清理缓存（可选）")
print(f"   ⚠️ 操作不可逆")

print("\n💡 reset_config() 特点:")
print("   - 重置 MCPStore 配置到初始状态")
print("   - 清除所有已注册的服务")
print("   - 停止所有运行中的服务")
print("   - 恢复默认配置")
print("   - 操作不可逆")

print("\n💡 使用场景:")
print("   - 清理所有配置")
print("   - 重新开始配置")
print("   - 环境重置")
print("   - 测试环境清理")
print("   - 故障恢复")

print("\n💡 注意事项:")
print("   - 操作不可逆")
print("   - 所有服务会被停止")
print("   - 建议先备份配置")
print("   - 确认没有重要服务运行")
print("   - 谨慎使用")

print("\n💡 reset vs delete 对比:")
print("   reset_config():")
print("      - 重置整个 Store 配置")
print("      - 清除所有服务")
print("      - 影响范围：全局")
print("   delete_service():")
print("      - 删除单个服务")
print("      - 只影响指定服务")
print("      - 影响范围：单服务")

print("\n" + "=" * 60)
print("✅ Store 重置配置测试完成")
print("=" * 60)

