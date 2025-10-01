"""
测试：Store 工具调用 - With 会话模式
功能：测试使用 with 上下文管理器管理会话，自动清理资源
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
print("测试：Store 工具调用 - With 会话模式")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加 Playwright 服务
print("\n1️⃣ 初始化 Store 并添加 Playwright 服务")
store = MCPStore.setup_store(debug=False)
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

# 2️⃣ 使用 with 语句创建会话上下文
print("\n2️⃣ 使用 with 语句创建会话上下文")
print(f"   会话 ID: browser_session")
print(f"   特点: 自动管理资源，退出时自动清理")

# 3️⃣ 在会话上下文中执行操作
print("\n3️⃣ 在会话上下文中执行操作")
with store.for_store().with_session("browser_session") as session:
    print(f"✅ 会话上下文已创建: {session.session_id}")
    
    # 4️⃣ 绑定服务
    print("\n4️⃣ 绑定服务到会话")
    session.bind_service("playwright")
    print(f"✅ 服务已绑定")
    
    # 5️⃣ 第一次工具调用 - 导航到百度
    print("\n5️⃣ 第一次工具调用 - 导航到百度")
    print(f"   调用工具: playwright_browser_navigate")
    print(f"   参数: url='https://www.baidu.com'")
    
    try:
        result1 = store.for_store().use_tool(
            "playwright_browser_navigate",
            {"url": "https://www.baidu.com"},
            timeout=180
        )
        print(f"✅ 导航成功")
        result_str = str(result1)
        if len(result_str) > 150:
            print(f"   返回: {result_str[:150]}...")
        else:
            print(f"   返回: {result_str}")
    except Exception as e:
        print(f"❌ 导航失败: {e}")
    
    # 6️⃣ 等待页面加载
    print("\n6️⃣ 等待页面加载")
    print(f"   等待 5 秒...")
    time.sleep(5)
    
    # 7️⃣ 第二次工具调用 - 导航到 MCPStore 官网
    print("\n7️⃣ 第二次工具调用 - 导航到 MCPStore 官网")
    print(f"   调用工具: playwright_browser_navigate")
    print(f"   参数: url='https://www.mcpstore.wiki'")
    
    try:
        result2 = store.for_store().use_tool(
            "playwright_browser_navigate",
            {"url": "https://www.mcpstore.wiki"},
            timeout=120
        )
        print(f"✅ 导航成功")
        result_str = str(result2)
        if len(result_str) > 150:
            print(f"   返回: {result_str[:150]}...")
        else:
            print(f"   返回: {result_str}")
    except Exception as e:
        print(f"❌ 导航失败: {e}")
    
    # 8️⃣ 再次等待
    print("\n8️⃣ 等待页面加载")
    print(f"   等待 5 秒...")
    time.sleep(5)
    
    print("\n✅ 会话操作完成，即将退出上下文")


print("\n" + "=" * 60)
print("✅ Store 工具调用 - With 会话模式测试完成")
print("=" * 60)

