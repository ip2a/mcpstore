#!/usr/bin/env python3
"""
MCPStore与LangChain集成示例

展示如何将MCPStore的工具集成到LangChain Agent中使用。
"""

import asyncio
import os
from mcpstore import MCPStore

# 检查是否安装了LangChain相关包
try:
    from langchain_openai import ChatOpenAI
    from langchain.agents import AgentExecutor, create_openai_tools_agent
    from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False
    print("⚠️  LangChain相关包未安装，请运行: pip install langchain langchain-openai")

async def main():
    """主函数：演示MCPStore与LangChain的集成"""
    
    if not LANGCHAIN_AVAILABLE:
        print("❌ 无法运行LangChain集成示例，请先安装相关依赖")
        return
    
    print("===== MCPStore与LangChain集成示例 =====")
    
    # 1. 初始化MCPStore并获取工具
    print("\n1. 初始化MCPStore并获取工具")
    store = MCPStore.setup_store()
    
    # 注册服务
    await store.for_store().add_service()
    
    # 获取LangChain工具
    tools = await (
        store
        .for_store()
        .for_langchain()
        .list_tools()
    )
    
    print(f"   ✓ 获取到 {len(tools)} 个工具")
    
    # 2. 设置LangChain Agent
    print("\n2. 设置LangChain Agent")
    
    # 检查OpenAI API Key
    if not os.getenv("OPENAI_API_KEY"):
        print("⚠️  请设置OPENAI_API_KEY环境变量")
        print("   示例: export OPENAI_API_KEY='your-api-key-here'")
        return
    
    # 初始化LLM
    llm = ChatOpenAI(
        model="gpt-3.5-turbo",
        temperature=0
    )
    
    # 创建提示模板
    prompt = ChatPromptTemplate.from_messages([
        ("system", "你是一个有用的助手，可以使用各种工具来帮助用户。"),
        ("human", "{input}"),
        MessagesPlaceholder(variable_name="agent_scratchpad"),
    ])
    
    # 创建Agent
    agent = create_openai_tools_agent(llm, tools, prompt)
    agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
    
    print("   ✓ LangChain Agent创建成功")
    
    # 3. 测试Agent
    print("\n3. 测试Agent")
    
    test_queries = [
        "帮我搜索北京的天气信息",
        "查找三里屯附近的咖啡店",
        "计算1+1等于多少"
    ]
    
    for i, query in enumerate(test_queries, 1):
        print(f"\n   测试 {i}: {query}")
        try:
            result = await agent_executor.ainvoke({"input": query})
            print(f"   结果: {result['output'][:200]}...")
        except Exception as e:
            print(f"   错误: {e}")
    
    print("\n===== 集成示例完成 =====")

if __name__ == "__main__":
    asyncio.run(main())
