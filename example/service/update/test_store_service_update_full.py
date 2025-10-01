"""
测试：Store 完整更新服务配置
功能：测试使用 update_config() 完整替换服务配置
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
print("测试：Store 完整更新服务配置")
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

# 2️⃣ 获取初始配置
print("\n2️⃣ 获取初始配置")
service_proxy = store.for_store().find_service("weather")
initial_info = service_proxy.service_info()
print(f"📋 初始配置:")
print(f"   类型: {initial_info.get('type', 'N/A')}")
print(f"   配置: {initial_info.get('config', 'N/A')}")

# 3️⃣ 准备新的配置（完整替换）
print("\n3️⃣ 准备新的配置（完整替换）")
new_config = {
    "url": "https://mcpstore.wiki/mcp",
    "timeout": 60,
    "retry": 3
}
print(f"📝 新配置:")
print(json.dumps(new_config, indent=2, ensure_ascii=False))

# 4️⃣ 使用 update_config() 完整更新配置
print("\n4️⃣ 使用 update_config() 完整更新配置")
result = service_proxy.update_config(new_config)
print(f"✅ 配置更新成功")
print(f"   返回结果: {result}")

# 5️⃣ 获取更新后的配置
print("\n5️⃣ 获取更新后的配置")
updated_info = service_proxy.service_info()
print(f"📋 更新后配置:")
print(f"   类型: {updated_info.get('type', 'N/A')}")
print(f"   配置: {updated_info.get('config', 'N/A')}")

# 6️⃣ 对比更新前后的配置
print("\n6️⃣ 对比更新前后的配置")
print(f"   初始配置: {initial_info.get('config', {})}")
print(f"   新配置: {updated_info.get('config', {})}")

# 7️⃣ 验证服务仍然可用
print("\n7️⃣ 验证服务仍然可用")
store.for_store().wait_service("weather", timeout=30.0)
tools = service_proxy.list_tools()
print(f"✅ 服务仍然可用")
print(f"   可用工具数量: {len(tools)}")

# 8️⃣ 再次更新配置（测试多次更新）
print("\n8️⃣ 再次更新配置（测试多次更新）")
new_config2 = {
    "url": "https://mcpstore.wiki/mcp",
    "timeout": 90,
    "retry": 5,
    "cache": True
}
result2 = service_proxy.update_config(new_config2)
print(f"✅ 第二次配置更新成功")

final_info = service_proxy.service_info()
print(f"📋 最终配置: {final_info.get('config', {})}")

print("\n💡 update_config() 特点:")
print("   - 完整替换服务配置")
print("   - 旧配置会被完全覆盖")
print("   - 适合重新配置服务")
print("   - 需要提供完整的新配置")
print("   - 更新后服务可能需要重启")

print("\n💡 使用场景:")
print("   - 切换服务URL")
print("   - 重新配置服务参数")
print("   - 配置迁移")
print("   - 环境切换（开发/生产）")

print("\n💡 注意事项:")
print("   - 确保新配置完整且正确")
print("   - 更新后可能需要 wait_service()")
print("   - 建议先备份原配置")
print("   - 大改动建议使用 restart_service()")

print("\n" + "=" * 60)
print("✅ Store 完整更新服务配置测试完成")
print("=" * 60)

