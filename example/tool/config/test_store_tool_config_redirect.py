"""
测试：Store 设置工具重定向
功能：测试使用 set_redirect() 设置工具重定向行为
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
print("测试：Store 设置工具重定向")
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

# 2️⃣ 查找工具
print("\n2️⃣ 查找工具")
tool_name = "get_current_weather"
tool_proxy = store.for_store().find_tool(tool_name)
print(f"✅ 找到工具: {tool_name}")

# 3️⃣ 检查初始重定向状态
print("\n3️⃣ 检查初始重定向状态")
initial_redirect = tool_proxy.set_redirect()
print(f"✅ 获取初始重定向状态: {initial_redirect}")

# 4️⃣ 设置重定向为 True
print("\n4️⃣ 设置重定向为 True")
tool_proxy.set_redirect(True)
redirect_status = tool_proxy.set_redirect()
print(f"✅ 重定向已设置为: {redirect_status}")

# 5️⃣ 测试重定向行为
print("\n5️⃣ 测试重定向行为")
params = {"query": "北京"}
print(f"   调用参数: {json.dumps(params, ensure_ascii=False)}")

# 调用工具并观察行为
result = tool_proxy.call_tool(params)
print(f"✅ 工具调用完成")
print(f"   返回类型: {type(result)}")

# 6️⃣ 设置重定向为 False
print("\n6️⃣ 设置重定向为 False")
tool_proxy.set_redirect(False)
redirect_status = tool_proxy.set_redirect()
print(f"✅ 重定向已设置为: {redirect_status}")

# 7️⃣ 测试非重定向行为
print("\n7️⃣ 测试非重定向行为")
result2 = tool_proxy.call_tool(params)
print(f"✅ 工具调用完成")
print(f"   返回类型: {type(result2)}")

# 8️⃣ 对比重定向和非重定向的结果
print("\n8️⃣ 对比重定向和非重定向的结果")
print(f"   重定向=True 的结果类型: {type(result)}")
print(f"   重定向=False 的结果类型: {type(result2)}")

if result == result2:
    print(f"   ✅ 重定向设置不影响结果内容")
else:
    print(f"   ⚠️ 重定向设置影响结果内容")

# 9️⃣ 测试多个工具的重定向设置
print("\n9️⃣ 测试多个工具的重定向设置")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    for tool in tools[:2]:
        proxy = store.for_store().find_tool(tool.name)
        
        # 设置重定向
        proxy.set_redirect(True)
        redirect_status = proxy.set_redirect()
        print(f"   工具 {tool.name} 重定向状态: {redirect_status}")
        
        # 重置为 False
        proxy.set_redirect(False)
        redirect_status = proxy.set_redirect()
        print(f"   工具 {tool.name} 重定向状态: {redirect_status}")

# 🔟 重定向的用途说明
print("\n🔟 重定向的用途说明")
print(f"   重定向功能用于:")
print(f"   - LangChain return_direct 行为")
print(f"   - 直接返回工具结果")
print(f"   - 跳过中间处理步骤")
print(f"   - 优化工具链性能")
print(f"   - 控制结果处理流程")

print("\n💡 set_redirect() 特点:")
print("   - 设置工具重定向行为")
print("   - 支持 True/False 切换")
print("   - 影响工具调用结果处理")
print("   - 用于框架集成优化")
print("   - 支持动态配置")

print("\n💡 使用场景:")
print("   - LangChain 集成")
print("   - 工具链优化")
print("   - 结果处理控制")
print("   - 性能优化")
print("   - 框架适配")

print("\n" + "=" * 60)
print("✅ Store 设置工具重定向测试完成")
print("=" * 60)

