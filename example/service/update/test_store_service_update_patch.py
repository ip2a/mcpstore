"""
测试：Store 增量更新服务配置
功能：测试使用 patch_config() 增量更新服务配置
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
print("测试：Store 增量更新服务配置")
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
initial_config = initial_info.get('config', {})
print(f"📋 初始配置:")
print(json.dumps(initial_config, indent=2, ensure_ascii=False))

# 3️⃣ 准备增量配置（只修改部分字段）
print("\n3️⃣ 准备增量配置（只修改部分字段）")
patch_config = {
    "timeout": 60
}
print(f"📝 增量配置:")
print(json.dumps(patch_config, indent=2, ensure_ascii=False))

# 4️⃣ 使用 patch_config() 增量更新配置
print("\n4️⃣ 使用 patch_config() 增量更新配置")
result = service_proxy.patch_config(patch_config)
print(f"✅ 配置增量更新成功")
print(f"   返回结果: {result}")

# 5️⃣ 获取更新后的配置
print("\n5️⃣ 获取更新后的配置")
patched_info = service_proxy.service_info()
patched_config = patched_info.get('config', {})
print(f"📋 更新后配置:")
print(json.dumps(patched_config, indent=2, ensure_ascii=False))

# 6️⃣ 对比配置变化
print("\n6️⃣ 对比配置变化")
print(f"   初始配置: {initial_config}")
print(f"   增量配置: {patch_config}")
print(f"   更新后配置: {patched_config}")
print(f"   ✅ 原有字段保留，新字段已添加")

# 7️⃣ 继续增量添加更多字段
print("\n7️⃣ 继续增量添加更多字段")
patch_config2 = {
    "retry": 3,
    "cache": True
}
result2 = service_proxy.patch_config(patch_config2)
print(f"✅ 第二次增量更新成功")

final_info = service_proxy.service_info()
final_config = final_info.get('config', {})
print(f"📋 最终配置:")
print(json.dumps(final_config, indent=2, ensure_ascii=False))

# 8️⃣ 修改已存在的字段
print("\n8️⃣ 修改已存在的字段")
patch_config3 = {
    "timeout": 90  # 修改之前添加的 timeout
}
result3 = service_proxy.patch_config(patch_config3)
print(f"✅ 修改已存在字段成功")

modified_info = service_proxy.service_info()
modified_config = modified_info.get('config', {})
print(f"📋 修改后配置:")
print(f"   timeout: {initial_config.get('timeout', '未设置')} → {patch_config.get('timeout')} → {modified_config.get('timeout', 'N/A')}")

# 9️⃣ 验证服务仍然可用
print("\n9️⃣ 验证服务仍然可用")
store.for_store().wait_service("weather", timeout=30.0)
tools = service_proxy.list_tools()
print(f"✅ 服务仍然可用")
print(f"   可用工具数量: {len(tools)}")

print("\n💡 patch_config() 特点:")
print("   - 增量更新服务配置")
print("   - 只修改指定的字段")
print("   - 未指定的字段保持不变")
print("   - 适合微调配置")
print("   - 支持添加新字段和修改已有字段")

print("\n💡 update_config() vs patch_config():")
print("   update_config():")
print("      - 完整替换配置")
print("      - 需要提供完整配置")
print("      - 旧字段会被删除")
print("      - 适合重新配置")
print("   patch_config():")
print("      - 增量更新配置")
print("      - 只需提供要修改的字段")
print("      - 旧字段保留")
print("      - 适合微调")

print("\n💡 使用场景:")
print("   - 调整超时时间")
print("   - 添加缓存配置")
print("   - 修改重试次数")
print("   - 启用/禁用特定功能")
print("   - 动态配置调整")

print("\n" + "=" * 60)
print("✅ Store 增量更新服务配置测试完成")
print("=" * 60)

