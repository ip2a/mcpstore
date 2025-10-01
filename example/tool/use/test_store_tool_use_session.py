"""
测试：Store 工具调用 - 会话模式
功能：测试使用会话模式调用工具，保持状态持久化
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
print("测试：Store 工具调用 - 会话模式")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加 Playwright 服务
print("\n1️⃣ 初始化 Store 并添加 Playwright 服务")
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "playwright": {
            "command": "npx",
            "args": ["@playwright/mcp"]
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("playwright", timeout=30.0)
print(f"✅ 服务 'playwright' 已添加并就绪")

# 2️⃣ 查看可用工具
print("\n2️⃣ 查看可用工具")
tools = store.for_store().list_tools()
tool_names = [t.name for t in tools]
print(f"✅ 可用工具数量: {len(tools)}")
print(f"   工具列表: {tool_names[:5]}..." if len(tool_names) > 5 else f"   工具列表: {tool_names}")

# 3️⃣ 创建会话并绑定服务
print("\n3️⃣ 创建会话并绑定服务")
session = store.for_store().create_session("playwright_test_session")
print(f"✅ 会话已创建: {session.session_id}")

try:
    session.bind_service("playwright")
    print(f"✅ 服务已绑定到会话")
except Exception as e:
    print(f"⚠️ 绑定服务可选（会在首次调用时自动创建）: {e}")

# 4️⃣ 第一次工具调用 - 导航到百度
print("\n4️⃣ 第一次工具调用 - 导航到百度")
print(f"   调用工具: playwright_browser_navigate")
print(f"   参数: url='https://www.baidu.com'")

start_time = time.perf_counter()
try:
    result1 = session.use_tool(
        "playwright_browser_navigate",
        {"url": "https://www.baidu.com"},
        timeout=180
    )
    end_time = time.perf_counter()
    
    print(f"✅ 第一次调用成功")
    print(f"   耗时: {(end_time - start_time):.3f} 秒")
    
    # 展示返回结果（简短版本）
    result_str = str(result1)
    if len(result_str) > 200:
        print(f"   返回结果: {result_str[:200]}...")
    else:
        print(f"   返回结果: {result_str}")
except Exception as e:
    print(f"❌ 第一次调用失败: {e}")
    exit(1)

# 5️⃣ 等待页面加载
print("\n5️⃣ 等待页面加载")
print(f"   等待 3 秒确保页面加载完成...")
time.sleep(3)

# 6️⃣ 第二次工具调用 - 获取页面快照（测试状态持久化）
print("\n6️⃣ 第二次工具调用 - 获取页面快照")
print(f"   调用工具: playwright_browser_snapshot")
print(f"   测试目的: 验证会话状态是否保持")

start_time = time.perf_counter()
try:
    result2 = session.use_tool(
        "playwright_browser_snapshot",
        {"input": ""},
        timeout=180
    )
    end_time = time.perf_counter()
    
    print(f"✅ 第二次调用成功")
    print(f"   耗时: {(end_time - start_time):.3f} 秒")
    
    # 展示返回结果（简短版本）
    result_str = str(result2)
    if len(result_str) > 200:
        print(f"   返回结果: {result_str[:200]}...")
    else:
        print(f"   返回结果: {result_str}")
except Exception as e:
    print(f"❌ 第二次调用失败: {e}")
    exit(1)

# 7️⃣ 验证状态持久化
print("\n7️⃣ 验证状态持久化")
result2_str = str(result2)
if "baidu.com" in result2_str:
    print(f"✅ 状态持久化成功: 快照中包含 'baidu.com'")
    print(f"   说明: 第二次调用时浏览器仍停留在百度页面")
elif "about:blank" in result2_str:
    print(f"❌ 状态持久化失败: 快照显示 'about:blank'")
    print(f"   说明: 会话状态未保持")
else:
    print(f"⚠️ 状态持久化结果不确定")
    print(f"   说明: 无法从快照中判断页面状态")

# 8️⃣ 会话信息
print("\n8️⃣ 会话信息")
print(f"   会话 ID: {session.session_id}")
print(f"   绑定服务: playwright")
print(f"   工具调用次数: 2")



print("\n" + "=" * 60)
print("✅ Store 工具调用 - 会话模式测试完成")
print("=" * 60)

