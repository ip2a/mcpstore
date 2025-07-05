#!/usr/bin/env python3
"""
MCPStore 综合功能示例
展示所有新实现的高优先级和中优先级功能
"""

import asyncio
import time
from mcpstore import ToolStore, create_tool_store

async def main():
    """主示例函数"""
    print("🚀 MCPStore 综合功能演示")
    print("=" * 60)
    
    # 1. 创建工具商店
    print("\n1️⃣ 创建工具商店")
    store = create_tool_store()
    print("   ✅ 工具商店创建成功")
    
    # 2. 添加服务
    print("\n2️⃣ 添加服务")
    services_to_add = ["mcpstore-demo-weather"]
    
    for service in services_to_add:
        success = store.add_service(service)
        if success:
            print(f"   ✅ 成功添加服务: {service}")
        else:
            print(f"   ❌ 添加服务失败: {service}")
    
    # 3. 查看可用工具
    print("\n3️⃣ 查看可用工具")
    tools = store.get_available_tools()
    print(f"   📋 找到 {len(tools)} 个工具:")
    
    for i, tool in enumerate(tools[:3]):  # 只显示前3个
        print(f"     {i+1}. {tool['name']}")
        print(f"        服务: {tool['service']}")
        print(f"        分类: {tool['category']}")
        print(f"        增强: {'是' if tool['is_enhanced'] else '否'}")
        print(f"        描述: {tool['description'][:50]}...")
        print()
    
    # 4. 工具转换功能演示
    print("\n4️⃣ 工具转换功能演示")
    if tools:
        original_tool = tools[0]['name']
        print(f"   🔧 为工具 '{original_tool}' 创建简化版本")
        
        try:
            simple_tool = store.create_simple_tool(original_tool, "simple_weather")
            print(f"   ✅ 创建简化工具: {simple_tool}")
        except Exception as e:
            print(f"   ⚠️ 创建简化工具失败: {e}")
        
        # 创建安全版本
        print(f"   🔒 为工具 '{original_tool}' 创建安全版本")
        try:
            validation_rules = {
                "city": {
                    "min_length": 2,
                    "max_length": 50,
                    "pattern": r"^[a-zA-Z\s]+$"
                }
            }
            safe_tool = store.create_safe_tool(original_tool, validation_rules)
            print(f"   ✅ 创建安全工具: {safe_tool}")
        except Exception as e:
            print(f"   ⚠️ 创建安全工具失败: {e}")
    
    # 5. 环境管理演示
    print("\n5️⃣ 环境管理演示")
    
    # 切换到开发环境
    print("   🔄 切换到开发环境")
    dev_success = store.switch_environment("development")
    print(f"   {'✅' if dev_success else '❌'} 开发环境切换: {'成功' if dev_success else '失败'}")
    
    # 创建自定义环境
    print("   🏗️ 创建自定义环境")
    custom_success = store.create_custom_environment("demo", ["weather", "general"])
    print(f"   {'✅' if custom_success else '❌'} 自定义环境创建: {'成功' if custom_success else '失败'}")
    
    # 6. 工具使用演示（带缓存和监控）
    print("\n6️⃣ 工具使用演示")
    if tools:
        weather_tools = [t for t in tools if "weather" in t['name'].lower()]
        if weather_tools:
            tool_name = weather_tools[0]['name']
            print(f"   🛠️ 使用工具: {tool_name}")
            
            # 第一次调用
            print("   📞 第一次调用（无缓存）")
            result1 = store.use_tool(tool_name, {"city": "Beijing"})
            print(f"   结果: 成功={result1['success']}, 缓存={result1.get('cached', False)}")
            print(f"   执行时间: {result1['execution_time']:.3f}秒")
            
            # 第二次调用（应该使用缓存）
            print("   📞 第二次调用（应该使用缓存）")
            result2 = store.use_tool(tool_name, {"city": "Beijing"})
            print(f"   结果: 成功={result2['success']}, 缓存={result2.get('cached', False)}")
            print(f"   执行时间: {result2['execution_time']:.3f}秒")
    
    # 7. OpenAPI 集成演示
    print("\n7️⃣ OpenAPI 集成演示")
    print("   🌐 导入示例 API（模拟）")
    try:
        # 这里使用一个公开的 OpenAPI 规范作为示例
        api_result = await store.import_api(
            "https://petstore.swagger.io/v2/swagger.json",
            "petstore_demo"
        )
        if api_result['success']:
            print(f"   ✅ API 导入成功: {api_result['tools_created']} 个工具")
        else:
            print(f"   ❌ API 导入失败: {api_result.get('error', '未知错误')}")
    except Exception as e:
        print(f"   ⚠️ API 导入演示跳过: {e}")
    
    # 8. 监控和分析演示
    print("\n8️⃣ 监控和分析演示")
    
    # 获取使用统计
    print("   📊 获取使用统计")
    stats = store.get_usage_stats()
    print(f"   总工具数: {stats['overview']['total_tools']}")
    print(f"   总服务数: {stats['overview']['total_services']}")
    print(f"   最近错误: {stats['overview']['recent_errors']}")
    
    if stats['top_tools']:
        print("   🏆 最常用工具:")
        for i, tool in enumerate(stats['top_tools'][:3]):
            print(f"     {i+1}. {tool['tool_name']} (调用 {tool['total_calls']} 次)")
    
    # 获取性能报告
    print("   ⚡ 获取性能报告")
    perf_report = store.get_performance_report()
    if perf_report['tool_cache']:
        cache_info = perf_report['tool_cache']
        print(f"   缓存命中率: {cache_info['hit_rate']:.2%}")
        print(f"   缓存条目数: {cache_info['entries']}")
        print(f"   内存使用: {cache_info['memory_usage']} 字节")
    
    # 9. 服务管理演示
    print("\n9️⃣ 服务管理演示")
    
    # 列出所有服务
    print("   📋 列出所有服务")
    services = store.list_services()
    for service in services:
        print(f"     • {service['name']} - 状态: {service['status']}")
    
    print("\n🎉 综合功能演示完成！")
    print("=" * 60)
    
    # 10. 功能总结
    print("\n📝 新功能总结:")
    print("✅ 工具转换功能 - 创建简化和安全版本的工具")
    print("✅ 组件控制 - 环境管理和工具过滤")
    print("✅ OpenAPI 集成 - 自动导入外部 API")
    print("✅ 认证安全 - Bearer Token 和 API Key 支持")
    print("✅ 智能缓存 - 工具结果缓存和性能优化")
    print("✅ 监控分析 - 使用统计和性能监控")
    print("✅ 客户友好 API - 直观易用的接口")
    print("✅ 现代化架构 - 删除旧格式，拥抱最新标准")

if __name__ == "__main__":
    asyncio.run(main())
