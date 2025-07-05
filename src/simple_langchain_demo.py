#!/usr/bin/env python3
"""
极致简单的 LangChain + MCPStore 演示
展示如何用最少的代码实现 AI Agent 调用 MCP 工具
"""

import asyncio
from mcpstore import MCPStore

def simple_demo():
    """最简单的演示 - 同步版本"""
    print("🚀 极致简单的 LangChain + MCPStore 演示")
    print("=" * 50)
    
    # 1. 初始化 MCPStore
    print("\n1️⃣ 初始化 MCPStore")
    store = MCPStore.setup_store()
    store.for_store().add_service()
    print("   ✅ MCPStore 初始化完成")
    
    # 2. 获取 LangChain 工具
    print("\n2️⃣ 转换为 LangChain 工具")
    langchain_tools = store.for_store().to_langchain_tools()
    print(f"   📋 获得 {len(langchain_tools)} 个 LangChain 工具")
    
    # 3. 展示工具信息
    print("\n3️⃣ 可用工具列表:")
    for i, tool in enumerate(langchain_tools[:3], 1):  # 只显示前3个
        print(f"   {i}. {tool.name}")
        print(f"      描述: {tool.description.split('。')[0]}。")
    
    # 4. 直接调用工具（不使用 LLM）
    print("\n4️⃣ 直接调用工具测试:")
    if langchain_tools:
        tool = langchain_tools[0]
        print(f"   🛠️ 测试工具: {tool.name}")
        
        try:
            # 直接调用工具
            result = tool.invoke({"query": "北京"})
            print(f"   ✅ 调用成功!")
            print(f"   📊 结果: {result}")
        except Exception as e:
            print(f"   ❌ 调用失败: {e}")
    
    print("\n🎉 基础演示完成!")

def agent_demo():
    """使用 LangChain Agent 的演示"""
    print("\n" + "=" * 50)
    print("🤖 LangChain Agent 演示")
    print("=" * 50)
    
    try:
        from langchain.agents import create_tool_calling_agent, AgentExecutor
        from langchain_core.prompts import ChatPromptTemplate
        from langchain_openai import ChatOpenAI
        
        print("\n1️⃣ 初始化组件")
        
        # 初始化 MCPStore
        store = MCPStore.setup_store()
        store.for_store().add_service()
        
        # 获取工具
        tools = store.for_store().to_langchain_tools()
        print(f"   📋 加载了 {len(tools)} 个工具")
        
        # 创建 LLM（需要设置 OpenAI API Key）
        try:
            llm = ChatOpenAI(
                temperature=0, model="deepseek-chat",
                openai_api_key="sk-bfcc353585a1456786a765b951c9842a",
                openai_api_base="https://api.deepseek.com"
            )
            print("   🧠 LLM 初始化成功")
        except Exception as e:
            print(f"   ⚠️ LLM 初始化失败: {e}")
            print("   💡 请设置 OPENAI_API_KEY 环境变量")
            return
        
        # 创建 Prompt
        prompt = ChatPromptTemplate.from_messages([
            ("system", "你是一个有用的助手，可以查询天气信息。"),
            ("human", "{input}"),
            ("placeholder", "{agent_scratchpad}"),
        ])
        
        # 创建 Agent
        agent = create_tool_calling_agent(llm, tools, prompt)
        agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
        
        print("\n2️⃣ Agent 创建成功!")
        
        # 测试查询
        print("\n3️⃣ 测试 AI Agent 调用:")
        test_queries = [
            "北京的天气怎么样？",
            "上海今天的天气如何？"
        ]
        
        for query in test_queries:
            print(f"\n   🤔 用户问题: {query}")
            try:
                response = agent_executor.invoke({"input": query})
                print(f"   🤖 AI 回答: {response['output']}")
            except Exception as e:
                print(f"   ❌ 执行失败: {e}")
        
        print("\n🎉 Agent 演示完成!")
        
    except ImportError as e:
        print(f"\n❌ 缺少依赖: {e}")
        print("💡 请安装: pip install langchain langchain-openai")

def async_demo():
    """异步版本演示"""
    print("\n" + "=" * 50)
    print("⚡ 异步版本演示")
    print("=" * 50)
    
    async def async_main():
        # 初始化
        store = MCPStore.setup_store()
        await store.for_store().add_service_async()
        
        # 获取工具
        tools = await store.for_store().to_langchain_tools_async()
        print(f"   📋 异步获取了 {len(tools)} 个工具")
        
        # 测试异步调用
        if tools:
            tool = tools[0]
            print(f"   🛠️ 异步测试工具: {tool.name}")
            
            try:
                # 异步调用工具
                result = await tool.acoroutine({"query": "深圳"})
                print(f"   ✅ 异步调用成功!")
                print(f"   📊 结果: {result}")
            except Exception as e:
                print(f"   ❌ 异步调用失败: {e}")
    
    # 运行异步代码
    asyncio.run(async_main())
    print("\n🎉 异步演示完成!")

def main():
    """主演示函数"""
    print("🌟 MCPStore + LangChain 集成演示")
    print("展示如何用最少的代码实现 AI Agent 工具调用")
    
    # 基础演示
    simple_demo()
    
    # Agent 演示
    agent_demo()
    
    # 异步演示
    async_demo()
    
    print("\n" + "=" * 50)
    print("📝 总结:")
    print("1. MCPStore 可以轻松转换为 LangChain 工具")
    print("2. 支持同步和异步两种调用方式")
    print("3. 可以直接集成到 LangChain Agent 中")
    print("4. 只需几行代码就能实现 AI 工具调用")
    print("\n🎯 核心代码:")
    print("   store = MCPStore.setup_store()")
    print("   store.for_store().add_service()")
    print("   tools = store.for_store().to_langchain_tools()")
    print("   # 然后就可以在 LangChain 中使用这些工具了!")

if __name__ == "__main__":
    main()
