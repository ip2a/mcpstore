#!/usr/bin/env python3
"""
MCPStore 新功能演示
展示如何在原有的两级上下文链式调用中使用新功能
"""

import asyncio
import time
from mcpstore import MCPStore

def main():
    """主演示函数"""
    print("🚀 MCPStore 新功能演示")
    print("=" * 60)
    print("保持原有设计：MCPStore.setup_store() + store.for_store() / store.for_agent()")
    print("=" * 60)
    
    # 1. 使用原有的设计模式
    print("\n1️⃣ 初始化 MCPStore（原有设计）")
    store = MCPStore.setup_store()
    print("   ✅ MCPStore 初始化成功")
    
    # 2. Store 级别的链式调用 + 新功能
    print("\n2️⃣ Store 级别链式调用 + 新功能")
    store_context = store.for_store()
    
    # 添加服务（原有功能）
    try:
        store_context.add_service(["mcpstore-demo-weather"])
        print("   ✅ 添加服务成功")
    except Exception as e:
        print(f"   ⚠️ 添加服务失败: {e}")
    
    # 启用智能缓存（新功能）
    store_context.enable_caching({
        "weather": 300,      # 天气工具缓存5分钟
        "search": 1800,      # 搜索工具缓存30分钟
    })
    print("   ✅ 启用智能缓存成功")
    
    # 设置认证（新功能）
    store_context.setup_auth("bearer", enabled=False)
    print("   ✅ 设置认证成功")
    
    # 切换环境（新功能）
    store_context.switch_environment("development")
    print("   ✅ 切换到开发环境成功")
    
    # 3. 获取工具并创建增强版本
    print("\n3️⃣ 工具转换功能演示")
    try:
        tools = store_context.list_tools()
        if tools:
            original_tool = tools[0].name
            print(f"   🔧 原始工具: {original_tool}")
            
            # 创建简化版工具（新功能）
            store_context.create_simple_tool(original_tool, "simple_weather")
            print("   ✅ 创建简化工具成功")
            
            # 创建安全版工具（新功能）
            validation_rules = {
                "city": {
                    "min_length": 2,
                    "max_length": 50,
                    "pattern": r"^[a-zA-Z\s\u4e00-\u9fff]+$"  # 支持中英文
                }
            }
            store_context.create_safe_tool(original_tool, validation_rules)
            print("   ✅ 创建安全工具成功")
        else:
            print("   ⚠️ 没有找到工具")
    except Exception as e:
        print(f"   ❌ 工具转换失败: {e}")
    
    # 4. Agent 级别的链式调用 + 新功能
    print("\n4️⃣ Agent 级别链式调用 + 新功能")
    agent_id = "demo_agent"
    agent_context = store.for_agent(agent_id)
    
    # 为 Agent 添加专属服务（原有功能）
    try:
        agent_context.add_service({
            "name": "agent_exclusive_service",
            "url": "http://59.110.160.18:21923/mcp"
        })
        print(f"   ✅ Agent {agent_id} 添加专属服务成功")
    except Exception as e:
        print(f"   ⚠️ Agent 添加服务失败: {e}")
    
    # Agent 级别的环境管理（新功能）
    agent_context.create_custom_environment("agent_env", ["weather", "safe"])
    print(f"   ✅ Agent {agent_id} 创建自定义环境成功")
    
    # Agent 级别的缓存配置（新功能）
    agent_context.enable_caching({"weather": 600})  # Agent 专属缓存配置
    print(f"   ✅ Agent {agent_id} 启用专属缓存成功")
    
    # 5. 工具使用演示
    print("\n5️⃣ 工具使用演示")
    try:
        # Store 级别使用工具
        store_tools = store_context.list_tools()
        if store_tools:
            weather_tool = None
            for tool in store_tools:
                if "weather" in tool.name.lower():
                    weather_tool = tool
                    break
            
            if weather_tool:
                print(f"   🛠️ Store 级别使用工具: {weather_tool.name}")
                start_time = time.time()
                result = store_context.use_tool(weather_tool.name, {"query": "北京"})
                duration = time.time() - start_time
                
                # 记录执行情况（新功能）
                store_context.record_tool_execution(
                    weather_tool.name, 
                    duration, 
                    hasattr(result, 'success') and result.success
                )
                print(f"   ✅ Store 工具执行完成，耗时 {duration:.3f}s")
        
        # Agent 级别使用工具
        agent_tools = agent_context.list_tools()
        if agent_tools:
            agent_tool = agent_tools[0]
            print(f"   🛠️ Agent 级别使用工具: {agent_tool.name}")
            agent_result = agent_context.use_tool(agent_tool.name, {"query": "上海"})
            print(f"   ✅ Agent 工具执行完成")
    except Exception as e:
        print(f"   ❌ 工具使用失败: {e}")
    
    # 6. 监控和统计
    print("\n6️⃣ 监控和统计功能")
    try:
        # Store 级别统计
        store_stats = store_context.get_usage_stats()
        print(f"   📊 Store 级别统计: {store_stats['overview']['total_tools']} 个工具")
        
        # Agent 级别统计
        agent_stats = agent_context.get_usage_stats()
        print(f"   📊 Agent 级别统计: {agent_stats['overview']['total_tools']} 个工具")
        
        # 性能报告
        perf_report = store_context.get_performance_report()
        if perf_report.get('tool_cache'):
            cache_info = perf_report['tool_cache']
            print(f"   ⚡ 缓存命中率: {cache_info['hit_rate']:.2%}")
    except Exception as e:
        print(f"   ❌ 获取统计失败: {e}")
    
    # 7. 链式调用演示
    print("\n7️⃣ 链式调用演示")
    try:
        # Store 级别的链式调用
        (store.for_store()
         .enable_caching({"api": 300})
         .setup_auth("api_key", False)
         .switch_environment("production"))
        print("   ✅ Store 级别链式调用成功")
        
        # Agent 级别的链式调用
        (store.for_agent("chain_demo_agent")
         .enable_caching({"weather": 180})
         .create_custom_environment("chain_env", ["safe"]))
        print("   ✅ Agent 级别链式调用成功")
    except Exception as e:
        print(f"   ❌ 链式调用失败: {e}")
    
    print("\n🎉 新功能演示完成！")
    print("=" * 60)
    print("📝 新功能总结:")
    print("✅ 工具转换: context.create_simple_tool() / create_safe_tool()")
    print("✅ 环境管理: context.switch_environment() / create_custom_environment()")
    print("✅ 性能优化: context.enable_caching() / get_performance_report()")
    print("✅ 认证安全: context.setup_auth()")
    print("✅ 监控分析: context.get_usage_stats() / record_tool_execution()")
    print("✅ OpenAPI 集成: context.import_api() (需要异步环境)")
    print("✅ 完全兼容原有的两级上下文链式调用设计")

async def async_demo():
    """异步功能演示"""
    print("\n🔄 异步功能演示")
    store = MCPStore.setup_store()
    context = store.for_store()
    
    try:
        # OpenAPI 集成（异步）
        await context.import_api_async(
            "https://petstore.swagger.io/v2/swagger.json",
            "petstore_demo"
        )
        print("   ✅ 异步导入 OpenAPI 成功")
    except Exception as e:
        print(f"   ❌ 异步导入失败: {e}")

if __name__ == "__main__":
    # 同步演示
    main()
    
    # 异步演示
    try:
        asyncio.run(async_demo())
    except Exception as e:
        print(f"异步演示失败: {e}")
