"""
测试：LangChain 集成 - Agent 基础调用
功能：测试 LangChain Agent 使用 MCPStore 工具执行任务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：LangChain 集成 - Agent 基础调用")
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

# 2️⃣ 获取 LangChain 工具列表
print("\n2️⃣ 获取 LangChain 工具列表")
tools = store.for_store().for_langchain().list_tools()
print(f"✅ 已加载 {len(tools)} 个 LangChain 工具")
if tools:
    print(f"   工具示例:")
    for i, tool in enumerate(tools[:3], 1):
        tool_name = getattr(tool, 'name', f'Tool_{i}')
        tool_desc = getattr(tool, 'description', 'N/A')
        desc_short = tool_desc[:50] + "..." if len(tool_desc) > 50 else tool_desc
        print(f"   {i}. {tool_name}: {desc_short}")

# 3️⃣ 配置 LLM
print("\n3️⃣ 配置 LLM")
print(f"   模型: deepseek-chat")
print(f"   温度: 0 (更确定性)")
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
    print(f"   请检查 API Key 和网络连接")
    exit(1)

# 4️⃣ 创建 Agent
print("\n4️⃣ 创建 LangChain Agent")
prompt = ChatPromptTemplate.from_messages([
    ("system", "你有一些工具可以使用，尽可能使用这些工具来完成任务"),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}"),
])

try:
    agent = create_tool_calling_agent(llm, tools, prompt)
    agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
    print(f"✅ Agent 创建成功")
    print(f"   Agent 类型: Tool Calling Agent")
    print(f"   可用工具: {len(tools)} 个")
except Exception as e:
    print(f"❌ Agent 创建失败: {e}")
    exit(1)

# 5️⃣ 执行任务
print("\n5️⃣ 执行任务")
query = "第一步打开百度页面，第二步在搜索框里输入'蓝色电风扇'并搜索"
print(f"   🤔 用户提问: {query}")
print(f"\n" + "-" * 60)
print("Agent 执行过程:")
print("-" * 60)

try:
    response = agent_executor.invoke({"input": query})
    print("-" * 60)
    print(f"\n✅ 任务执行完成")
    print(f"   🤖 Agent 回复: {response['output']}")
except Exception as e:
    print(f"\n❌ 任务执行失败: {e}")
    print(f"   可能原因:")
    print(f"   - 工具调用超时")
    print(f"   - LLM API 限制")
    print(f"   - 网络问题")

# 6️⃣ Agent 特性说明
print("\n6️⃣ Agent 特性说明")
print(f"   - 自主决策：Agent 自动选择使用哪些工具")
print(f"   - 多步推理：可以执行多步骤的复杂任务")
print(f"   - 工具链：自动组合多个工具完成任务")
print(f"   - 错误恢复：遇到错误时尝试其他方案")

# 7️⃣ 性能建议
print("\n7️⃣ 性能建议")
print(f"   - 合理设置超时时间")
print(f"   - 使用会话模式保持状态")
print(f"   - 控制 Agent 的最大迭代次数")
print(f"   - 监控 LLM API 调用次数")

print("\n💡 LangChain Agent 特点:")
print("   - 智能工具选择：自动选择合适工具")
print("   - 自然语言交互：用自然语言描述任务")
print("   - 复杂任务处理：处理多步骤任务")
print("   - 灵活扩展：轻松添加新工具")

print("\n💡 使用场景:")
print("   - 浏览器自动化：网页操作、数据抓取")
print("   - 数据处理：复杂数据转换")
print("   - 工作流自动化：多步骤业务流程")
print("   - 智能助手：对话式任务执行")

print("\n" + "=" * 60)
print("✅ LangChain 集成 - Agent 基础调用测试完成")
print("=" * 60)

