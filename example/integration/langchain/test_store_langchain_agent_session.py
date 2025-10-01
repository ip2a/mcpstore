"""
测试：LangChain 集成 - Agent 会话模式
功能：测试 LangChain Agent 在会话上下文中使用工具，保持状态持久化
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：LangChain 集成 - Agent 会话模式")
print("=" * 60)

# 0️⃣ 检查依赖
print("\n0️⃣ 检查 LangChain 依赖")
try:
    from langchain.agents import create_tool_calling_agent, AgentExecutor
    from langchain_core.prompts import ChatPromptTemplate
    from langchain_openai import ChatOpenAI
    print(f"✅ LangChain 依赖已安装")
except ImportError as e:
    print(f"❌ 缺少依赖: {e}")
    print(f"   请安装: pip install langchain langchain-openai")
    exit(1)

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

# 2️⃣ 创建会话并绑定服务
print("\n2️⃣ 创建会话并绑定服务")
session = store.for_store().create_session("langchain_browser")
session.bind_service("playwright")
print(f"✅ 会话已创建: {session.session_id}")
print(f"   绑定服务: playwright")
print(f"   说明: 会话模式可以保持浏览器状态")

# 3️⃣ 使用 with 会话上下文
print("\n3️⃣ 使用 with 会话上下文")
with store.for_store().with_session(session.session_id) as s:
    print(f"✅ 进入会话上下文: {s.session_id}")
    
    # 4️⃣ 获取 LangChain 工具
    print("\n4️⃣ 获取 LangChain 工具")
    tools = store.for_store().for_langchain().list_tools()
    print(f"✅ 已加载 {len(tools)} 个 LangChain 工具")
    
    # 5️⃣ 配置 LLM
    print("\n5️⃣ 配置 LLM")
    print(f"   模型: deepseek-chat")
    try:
        llm = ChatOpenAI(
            temperature=0,
            model="deepseek-chat",
            openai_api_key="sk-24e1c752e6114950952365631d18cf4f",
            openai_api_base="https://api.deepseek.com",
        )
        print(f"✅ LLM 配置成功")
    except Exception as e:
        print(f"❌ LLM 配置失败: {e}")
        exit(1)
    
    # 6️⃣ 创建 Agent
    print("\n6️⃣ 创建 LangChain Agent")
    prompt = ChatPromptTemplate.from_messages([
        ("system", "你有一些工具可以使用，尽可能使用这些工具来完成任务"),
        ("human", "{input}"),
        ("placeholder", "{agent_scratchpad}"),
    ])
    
    try:
        agent = create_tool_calling_agent(llm, tools, prompt)
        agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
        print(f"✅ Agent 创建成功")
    except Exception as e:
        print(f"❌ Agent 创建失败: {e}")
        exit(1)
    
    # 7️⃣ 执行任务
    print("\n7️⃣ 执行任务")
    query = "第一步打开百度页面，第二步在搜索框里输入'蓝色电风扇'并搜索"
    print(f"   🤔 用户提问: {query}")
    print(f"\n" + "-" * 60)
    print("Agent 执行过程（在会话中）:")
    print("-" * 60)
    
    try:
        response = agent_executor.invoke({"input": query})
        print("-" * 60)
        print(f"\n✅ 任务执行完成")
        print(f"   🤖 Agent 回复: {response['output']}")
    except Exception as e:
        print(f"\n❌ 任务执行失败: {e}")
    
    print("\n8️⃣ 会话状态说明")
    print(f"   - 会话 ID: {s.session_id}")
    print(f"   - 浏览器状态: 保持在最后访问的页面")
    print(f"   - 优点: 可以继续在同一浏览器上下文操作")
    print(f"   - 说明: 如果需要继续操作，可以再次调用 Agent")

print("\n9️⃣ 会话已自动清理")
print(f"   说明: with 语句退出时自动清理了会话资源")

# 🔟 会话模式 vs 非会话模式对比
print("\n🔟 会话模式 vs 非会话模式对比")
print(f"\n   非会话模式:")
print(f"   - 每次调用创建新的浏览器实例")
print(f"   - 无法保持状态")
print(f"   - 适合独立的单次任务")
print(f"\n   会话模式:")
print(f"   - 共享同一个浏览器实例")
print(f"   - 保持页面状态和 Cookie")
print(f"   - 适合需要多步操作的任务")
print(f"   - 提高性能（避免重复初始化）")

print("\n💡 会话模式特点:")
print("   - 状态持久化：保持浏览器状态")
print("   - 性能优化：复用浏览器实例")
print("   - 上下文管理：自动资源清理")
print("   - Agent 友好：适合多步骤 Agent 任务")

print("\n💡 使用场景:")
print("   - 多步骤浏览器操作")
print("   - 需要登录的网站操作")
print("   - 复杂的页面交互流程")
print("   - Agent 执行长任务")

print("\n💡 最佳实践:")
print("   - 使用 with 语句管理会话")
print("   - 为会话使用有意义的名称")
print("   - 合理设置超时时间")
print("   - 监控会话资源使用")

print("\n" + "=" * 60)
print("✅ LangChain 集成 - Agent 会话模式测试完成")
print("=" * 60)

