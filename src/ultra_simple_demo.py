#!/usr/bin/env python3
"""
🌟 超级简单的 MCPStore + LangChain 演示
只需要 10 行代码就能实现 AI Agent 工具调用！
"""

from mcpstore import MCPStore

def main():
    """超级简单的演示 - 只要 10 行代码！"""
    print("🚀 超级简单演示：10 行代码实现 AI 工具调用")
    print("=" * 50)
    
    # ===== 核心代码开始 =====
    # 1. 初始化 MCPStore（1行）
    store = MCPStore.setup_store()
    
    # 2. 添加服务（1行）
    store.for_store().add_service()
    
    # 3. 转换为 LangChain 工具（1行）
    tools = store.for_store().to_langchain_tools()
    
    # 4. 使用工具（1行）
    result = tools[0].invoke({"query": "北京"})
    
    # 5. 显示结果（1行）
    print(f"🌤️ 天气结果: {result}")
    # ===== 核心代码结束 =====
    
    print("\n✨ 就这么简单！只需要 5 行核心代码！")
    
    # 详细信息展示
    print(f"\n📊 详细信息:")
    print(f"   🛠️ 可用工具数量: {len(tools)}")
    print(f"   📋 第一个工具名称: {tools[0].name}")
    print(f"   📝 工具描述: {tools[0].description.split('。')[0]}。")
    
    print(f"\n🎯 完整的可复制代码:")
    print("```python")
    print("from mcpstore import MCPStore")
    print("store = MCPStore.setup_store()")
    print("store.for_store().add_service()")
    print("tools = store.for_store().to_langchain_tools()")
    print('result = tools[0].invoke({"query": "北京"})')
    print("print(result)")
    print("```")

if __name__ == "__main__":
    main()
