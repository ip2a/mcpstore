#!/usr/bin/env python3
"""
特殊示例：Playwright 浏览器会话持久化
功能：演示浏览器状态在多次工具调用间的持久化
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from utils.cleanup_helper import print_and_reset_config
from mcpstore import MCPStore

print("=" * 60)
print("特殊示例：Playwright 浏览器会话持久化")
print("=" * 60)

# 初始化 Store（完整链式）
print("\n✅ 初始化 Store")
store = MCPStore.setup_store(debug=True)

# 添加 Playwright 服务（完整链式）
print("\n✅ 添加 Playwright 服务")
store.for_store().add_service({
    "mcpServers": {
        "playwright": {
            "command": "npx",
            "args": ["@playwright/mcp"]
        }
    }
})
store.for_store().wait_service("playwright", timeout=30)
print("   Playwright 服务添加完成")

# 第一次调用：导航到百度（完整链式）
print("\n✅ 第一次调用：导航到百度")
result1 = store.for_store().use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})
print(f"   导航完成，结果长度: {len(str(result1))}")
if "baidu.com" in str(result1):
    print("   ✅ 成功导航到百度")

# 第二次调用：获取页面快照（完整链式）
print("\n✅ 第二次调用：获取页面快照")
result2 = store.for_store().use_tool("playwright_browser_snapshot", {"input": ""})
print(f"   快照获取完成，结果长度: {len(str(result2))}")

# 验证会话持久化
if "baidu.com" in str(result2):
    print("   ✅ 浏览器状态保持，仍在百度页面")
    print("   🎉 会话持久化成功！")
elif "about:blank" in str(result2):
    print("   ❌ 浏览器状态丢失，页面重置为空白页")
else:
    print("   ⚠️ 页面状态不明确")

print("\n💡 会话持久化说明:")
print("   - MCPStore 会自动维护浏览器会话")
print("   - 多次工具调用共享同一个浏览器实例")
print("   - 无需手动管理会话生命周期")

print("\n" + "=" * 60)
print("✅ 示例完成！")
print("=" * 60)

# 清理配置
print_and_reset_config(store, "清理示例配置")

