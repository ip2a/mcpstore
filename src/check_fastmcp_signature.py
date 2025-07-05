#!/usr/bin/env python3
"""
检查 FastMCP 客户端的实际方法签名
"""

import inspect

def get_fastmcp_client_class():
    """获取 FastMCP 客户端类"""
    try:
        from fastmcp import Client
        return Client
    except ImportError:
        try:
            from fastmcp.client import Client
            return Client
        except ImportError:
            try:
                from fastmcp import FastMCPClient
                return FastMCPClient
            except ImportError:
                return None

def check_call_tool_signature():
    """检查 call_tool 方法的签名"""
    print("🔍 检查 FastMCP 客户端方法签名")

    # 获取客户端类
    ClientClass = get_fastmcp_client_class()
    if not ClientClass:
        print("❌ 无法找到 FastMCP 客户端类")
        return

    print(f"✅ 找到客户端类: {ClientClass}")

    # 获取 call_tool 方法的签名
    call_tool_method = getattr(ClientClass, 'call_tool', None)
    if call_tool_method:
        signature = inspect.signature(call_tool_method)
        print(f"\n📋 call_tool 方法签名:")
        print(f"   {signature}")
        
        print(f"\n📝 参数详情:")
        for param_name, param in signature.parameters.items():
            print(f"   - {param_name}: {param.annotation} = {param.default}")
        
        # 检查是否有 raise_on_error 参数
        if 'raise_on_error' in signature.parameters:
            print(f"\n✅ 支持 raise_on_error 参数")
            param = signature.parameters['raise_on_error']
            print(f"   类型: {param.annotation}")
            print(f"   默认值: {param.default}")
        else:
            print(f"\n❌ 不支持 raise_on_error 参数")
    else:
        print("❌ 找不到 call_tool 方法")
    
    # 检查其他相关方法
    print(f"\n🔍 检查其他工具相关方法:")
    methods = ['list_tools', 'call_tool_mcp']
    for method_name in methods:
        method = getattr(ClientClass, method_name, None)
        if method:
            signature = inspect.signature(method)
            print(f"   {method_name}: {signature}")
        else:
            print(f"   {method_name}: 不存在")

def check_fastmcp_version():
    """检查 FastMCP 版本信息"""
    try:
        import fastmcp
        print(f"\n📦 FastMCP 版本信息:")
        print(f"   版本: {fastmcp.__version__}")
        
        # 检查是否有版本相关的属性
        if hasattr(fastmcp, '__version__'):
            version = fastmcp.__version__
            print(f"   详细版本: {version}")
            
            # 解析版本号
            version_parts = version.split('.')
            if len(version_parts) >= 2:
                major, minor = int(version_parts[0]), int(version_parts[1])
                print(f"   主版本: {major}, 次版本: {minor}")
                
                if major >= 2 and minor >= 10:
                    print(f"   ✅ 版本支持 .data 属性 (需要 2.10.0+)")
                else:
                    print(f"   ⚠️ 版本可能不完全支持最新特性")
        
    except Exception as e:
        print(f"   ❌ 获取版本信息失败: {e}")

def test_actual_call():
    """测试实际调用"""
    print(f"\n🧪 测试实际调用:")
    
    try:
        from mcpstore import MCPStore
        
        # 初始化
        store = MCPStore.setup_store()
        store.for_store().add_service()
        
        # 获取工具
        tools = store.for_store().list_tools()
        if tools:
            tool = tools[0]
            print(f"   工具: {tool.name}")
            
            # 获取实际的客户端
            service_name = tool.service_name
            orchestrator = store.orchestrator
            
            # 检查客户端
            if hasattr(orchestrator, '_clients') and service_name in orchestrator._clients:
                client = orchestrator._clients[service_name]
                print(f"   客户端类型: {type(client)}")
                
                # 检查客户端的 call_tool 方法
                if hasattr(client, 'call_tool'):
                    method = getattr(client, 'call_tool')
                    signature = inspect.signature(method)
                    print(f"   实际客户端 call_tool 签名: {signature}")
                    
                    # 检查参数
                    params = list(signature.parameters.keys())
                    print(f"   支持的参数: {params}")
                    
                    if 'raise_on_error' in params:
                        print(f"   ✅ 实际客户端支持 raise_on_error")
                    else:
                        print(f"   ❌ 实际客户端不支持 raise_on_error")
                else:
                    print(f"   ❌ 客户端没有 call_tool 方法")
            else:
                print(f"   ❌ 找不到客户端")
        else:
            print(f"   ❌ 没有可用工具")
            
    except Exception as e:
        print(f"   ❌ 测试失败: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    check_fastmcp_version()
    check_call_tool_signature()
    test_actual_call()
