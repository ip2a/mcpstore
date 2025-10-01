"""
测试：Store 刷新服务内容
功能：测试使用 refresh_content() 刷新服务的工具列表等内容
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 刷新服务内容")
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

# 2️⃣ 获取刷新前的工具列表
print("\n2️⃣ 获取刷新前的工具列表")
service_proxy = store.for_store().find_service("weather")
tools_before = service_proxy.list_tools()
print(f"📋 刷新前工具数量: {len(tools_before)}")
if tools_before:
    print(f"   工具列表:")
    for tool in tools_before:
        print(f"   - {tool.name}")

# 3️⃣ 使用 refresh_content() 刷新服务内容
print("\n3️⃣ 使用 refresh_content() 刷新服务内容")
print(f"⏳ 正在刷新服务内容...")
result = service_proxy.refresh_content()
print(f"✅ 服务内容刷新完成")
print(f"   返回结果: {result}")

# 4️⃣ 获取刷新后的工具列表
print("\n4️⃣ 获取刷新后的工具列表")
tools_after = service_proxy.list_tools()
print(f"📋 刷新后工具数量: {len(tools_after)}")
if tools_after:
    print(f"   工具列表:")
    for tool in tools_after:
        print(f"   - {tool.name}")

# 5️⃣ 对比刷新前后的变化
print("\n5️⃣ 对比刷新前后的变化")
print(f"   刷新前工具数: {len(tools_before)}")
print(f"   刷新后工具数: {len(tools_after)}")

if len(tools_before) == len(tools_after):
    print(f"   ✅ 工具数量一致")
else:
    print(f"   ⚠️ 工具数量有变化")

# 6️⃣ 验证服务状态
print("\n6️⃣ 验证服务状态")
status = service_proxy.service_status()
print(f"📊 服务状态:")
print(f"   状态: {status.get('state', 'N/A')}")
print(f"   健康: {status.get('health', 'N/A')}")

# 7️⃣ 测试工具仍然可用
print("\n7️⃣ 测试工具仍然可用")
if tools_after:
    tool_name = "get_current_weather"
    result = store.for_store().use_tool(tool_name, {"query": "北京"})
    print(f"✅ 工具调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

# 8️⃣ 对比 refresh_content() 和 restart_service()
print("\n8️⃣ refresh_content() vs restart_service()")
print(f"\n   refresh_content():")
print(f"   - 只刷新内容（工具、资源、提示列表）")
print(f"   - 不重启服务进程")
print(f"   - 更轻量，更快速")
print(f"   - 服务持续运行")
print(f"\n   restart_service():")
print(f"   - 完全重启服务")
print(f"   - 重启进程，重新连接")
print(f"   - 耗时更长")
print(f"   - 服务会短暂中断")

# 9️⃣ 再次刷新内容
print("\n9️⃣ 再次刷新内容（测试多次刷新）")
result2 = service_proxy.refresh_content()
print(f"✅ 第二次刷新完成")

tools_final = service_proxy.list_tools()
print(f"📋 最终工具数量: {len(tools_final)}")

print("\n💡 refresh_content() 特点:")
print("   - 刷新服务的内容列表")
print("   - 不重启服务进程")
print("   - 重新获取工具、资源、提示")
print("   - 轻量级操作，速度快")
print("   - 服务保持运行状态")

print("\n💡 使用场景:")
print("   - 服务新增了工具")
print("   - 工具列表需要更新")
print("   - 服务端内容有变化")
print("   - 定期同步内容")
print("   - 不想重启但需要更新")

print("\n💡 何时使用 refresh vs restart:")
print("   使用 refresh_content():")
print("      - 只需要更新工具列表")
print("      - 服务运行正常")
print("      - 追求速度")
print("   使用 restart_service():")
print("      - 服务出现异常")
print("      - 配置有重大变更")
print("      - 需要完全重置")

print("\n" + "=" * 60)
print("✅ Store 刷新服务内容测试完成")
print("=" * 60)

