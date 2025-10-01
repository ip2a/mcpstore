"""
测试：Store 重启服务
功能：测试使用 restart_service() 重启服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import time

print("=" * 60)
print("测试：Store 重启服务")
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

# 2️⃣ 获取重启前的服务状态
print("\n2️⃣ 获取重启前的服务状态")
service_proxy = store.for_store().find_service("weather")
status_before = service_proxy.service_status()
print(f"📊 重启前状态:")
print(f"   状态: {status_before.get('state', 'N/A')}")
print(f"   健康: {status_before.get('health', 'N/A')}")
if 'uptime' in status_before:
    print(f"   运行时间: {status_before.get('uptime', 'N/A')} 秒")

# 3️⃣ 使用 restart_service() 重启服务
print("\n3️⃣ 使用 restart_service() 重启服务")
print(f"⏳ 正在重启服务...")
start_time = time.time()
result = service_proxy.restart_service()
elapsed_time = time.time() - start_time
print(f"✅ 服务重启完成")
print(f"   返回结果: {result}")
print(f"   耗时: {elapsed_time:.2f} 秒")

# 4️⃣ 等待服务重新就绪
print("\n4️⃣ 等待服务重新就绪")
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务已重新就绪")

# 5️⃣ 获取重启后的服务状态
print("\n5️⃣ 获取重启后的服务状态")
status_after = service_proxy.service_status()
print(f"📊 重启后状态:")
print(f"   状态: {status_after.get('state', 'N/A')}")
print(f"   健康: {status_after.get('health', 'N/A')}")
if 'uptime' in status_after:
    print(f"   运行时间: {status_after.get('uptime', 'N/A')} 秒")

# 6️⃣ 验证服务可用
print("\n6️⃣ 验证服务可用")
tools = service_proxy.list_tools()
print(f"✅ 服务可用")
print(f"   工具数量: {len(tools)}")

# 7️⃣ 测试工具调用
print("\n7️⃣ 测试工具调用")
if tools:
    tool_name = "get_current_weather"
    result = store.for_store().use_tool(tool_name, {"query": "北京"})
    print(f"✅ 工具调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

# 8️⃣ 再次重启（测试多次重启）
print("\n8️⃣ 再次重启（测试多次重启）")
result2 = service_proxy.restart_service()
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 第二次重启成功")

status_final = service_proxy.service_status()
print(f"📊 最终状态: {status_final.get('state', 'N/A')}")

print("\n💡 restart_service() 特点:")
print("   - 重启服务进程")
print("   - 重新建立连接")
print("   - 重新加载配置")
print("   - 重置运行时间")
print("   - 适合解决服务异常")

print("\n💡 使用场景:")
print("   - 服务出现异常")
print("   - 配置更新后生效")
print("   - 定期维护重启")
print("   - 内存泄漏恢复")
print("   - 连接问题修复")

print("\n💡 注意事项:")
print("   - 重启会短暂中断服务")
print("   - 需要等待服务重新就绪")
print("   - 建议在低峰期操作")
print("   - 重启后验证服务可用")

print("\n" + "=" * 60)
print("✅ Store 重启服务测试完成")
print("=" * 60)

