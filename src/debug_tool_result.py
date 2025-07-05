#!/usr/bin/env python3
"""
调试工具执行结果
"""

from mcpstore import MCPStore
import json

def debug_tool_execution():
    """调试工具执行"""
    print("🔍 调试工具执行结果")
    
    # 初始化
    store = MCPStore.setup_store()
    store.for_store().add_service()
    
    # 获取工具
    tools = store.for_store().list_tools()
    if not tools:
        print("❌ 没有找到工具")
        return
    
    tool = tools[0]
    print(f"🛠️ 使用工具: {tool.name}")
    print(f"📝 工具描述: {tool.description}")
    
    # 执行工具
    params = {"query": "北京"}
    print(f"📝 参数: {params}")
    
    try:
        result = store.for_store().use_tool(tool.name, params)
        
        print(f"\n📊 执行结果分析:")
        print(f"   类型: {type(result)}")
        print(f"   成功: {result.success}")
        print(f"   错误: {result.error}")
        print(f"   消息: {result.message}")
        print(f"   结果: {result.result}")
        
        # 如果结果是字典或对象，尝试序列化
        if result.result is not None:
            try:
                if hasattr(result.result, '__dict__'):
                    print(f"   结果属性: {vars(result.result)}")
                elif isinstance(result.result, (dict, list)):
                    print(f"   结果JSON: {json.dumps(result.result, indent=2, ensure_ascii=False)}")
                else:
                    print(f"   结果字符串: {str(result.result)}")
            except Exception as e:
                print(f"   结果序列化失败: {e}")
        
        # 显示工具信息
        print(f"\n🔧 工具信息:")
        print(f"   服务名: {tool.service_name}")
        print(f"   完整工具名: {tool.name}")
        if '_' in tool.name:
            tool_name_without_prefix = tool.name.split('_', 1)[1]
            print(f"   去前缀工具名: {tool_name_without_prefix}")
        else:
            print(f"   工具名无前缀")
        
    except Exception as e:
        print(f"❌ 工具执行失败: {e}")
        import traceback
        traceback.print_exc()

async def async_debug():
    """异步调试"""
    from mcpstore import MCPStore
    
    store = MCPStore.setup_store()
    store.for_store().add_service()
    
    tools = store.for_store().list_tools()
    if not tools:
        return
    
    tool = tools[0]
    service_name = tool.service_name
    tool_name_without_prefix = tool.name.split('_', 1)[1] if '_' in tool.name else tool.name
    
    print(f"\n🔧 异步直接调用:")
    print(f"   服务名: {service_name}")
    print(f"   工具名: {tool_name_without_prefix}")
    
    try:
        raw_result = await store.orchestrator.execute_tool_fastmcp(
            service_name=service_name,
            tool_name=tool_name_without_prefix,
            arguments={"query": "北京"}
        )
        
        print(f"   ✅ 异步调用成功")
        print(f"   结果类型: {type(raw_result)}")
        print(f"   结果内容: {raw_result}")
        
        if hasattr(raw_result, '__dict__'):
            print(f"   结果属性: {vars(raw_result)}")
        
    except Exception as e:
        print(f"   ❌ 异步调用失败: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    debug_tool_execution()
    
    # 运行异步测试
    import asyncio
    try:
        asyncio.run(async_debug())
    except Exception as e:
        print(f"异步测试失败: {e}")
