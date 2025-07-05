#!/usr/bin/env python3
"""
调试MCP服务注册问题
测试批量添加服务API的具体响应
"""

import sys
import os
import json

sys.path.append(os.path.dirname(os.path.abspath(__file__)))

def test_batch_add_services_api():
    """测试批量添加服务API"""
    print("🧪 测试批量添加服务API...")
    
    try:
        from utils.api_client import MCPStoreAPI
        
        api_client = MCPStoreAPI("http", "http://localhost:18611")
        
        if not api_client.test_connection():
            print("    ❌ API服务器未连接")
            return False
        
        # 测试数据
        test_services = [
            {
                "name": "debug_test_service_1",
                "url": "http://example1.com/mcp",
                "description": "调试测试服务1",
                "transport": "auto"
            },
            {
                "name": "debug_test_service_2",
                "url": "http://example2.com/mcp", 
                "description": "调试测试服务2",
                "transport": "sse"
            }
        ]
        
        print(f"    📤 发送请求: {len(test_services)} 个服务")
        print(f"    📋 服务配置:")
        for service in test_services:
            print(f"      - {service['name']}: {service['url']}")
        
        # 调用API
        response = api_client.batch_add_services(test_services)
        
        print(f"    📥 API响应:")
        print(f"      - 响应类型: {type(response)}")
        print(f"      - 响应内容: {response}")
        
        if response:
            success = response.get('success', False)
            print(f"      - 成功标志: {success}")
            
            if success:
                data = response.get('data', {})
                summary = data.get('summary', {})
                results = data.get('results', [])
                
                print(f"      - 数据部分: {data}")
                print(f"      - 摘要信息: {summary}")
                print(f"      - 结果详情: {len(results)} 条")
                
                for result in results:
                    name = result.get('name', 'Unknown')
                    result_success = result.get('success', False)
                    error = result.get('error', '')
                    print(f"        * {name}: {'成功' if result_success else f'失败 - {error}'}")
            else:
                message = response.get('message', '无错误信息')
                print(f"      - 错误信息: {message}")
        else:
            print("      - 响应为空或None")
        
        return True
        
    except Exception as e:
        print(f"    ❌ 测试失败: {e}")
        import traceback
        print(f"    📋 详细错误: {traceback.format_exc()}")
        return False

def test_single_add_service_api():
    """测试单个添加服务API作为对比"""
    print("\n🔧 测试单个添加服务API...")
    
    try:
        from utils.api_client import MCPStoreAPI
        
        api_client = MCPStoreAPI("http", "http://localhost:18611")
        
        # 测试单个服务
        test_service = {
            "name": "debug_single_test_service",
            "url": "http://single.example.com/mcp",
            "description": "单个调试测试服务"
        }
        
        print(f"    📤 发送单个服务请求: {test_service['name']}")
        
        response = api_client.add_service(test_service)
        
        print(f"    📥 单个服务API响应:")
        print(f"      - 响应类型: {type(response)}")
        print(f"      - 响应内容: {response}")
        
        if response:
            success = response.get('success', False)
            message = response.get('message', '')
            print(f"      - 成功标志: {success}")
            print(f"      - 消息: {message}")
        
        return True
        
    except Exception as e:
        print(f"    ❌ 单个服务测试失败: {e}")
        return False

def test_api_client_methods():
    """测试API客户端方法"""
    print("\n🔍 测试API客户端方法...")
    
    try:
        from utils.api_client import MCPStoreAPI
        
        api_client = MCPStoreAPI("http", "http://localhost:18611")
        
        # 检查方法是否存在
        methods_to_check = [
            'batch_add_services',
            'add_service',
            'list_services',
            'test_connection'
        ]
        
        for method_name in methods_to_check:
            if hasattr(api_client, method_name):
                method = getattr(api_client, method_name)
                print(f"    ✅ {method_name}: {type(method)}")
            else:
                print(f"    ❌ {method_name}: 方法不存在")
        
        return True
        
    except Exception as e:
        print(f"    ❌ API客户端方法测试失败: {e}")
        return False

def test_current_services():
    """测试获取当前服务列表"""
    print("\n📋 测试获取当前服务列表...")
    
    try:
        from utils.api_client import MCPStoreAPI
        
        api_client = MCPStoreAPI("http", "http://localhost:18611")
        
        response = api_client.list_services()
        
        print(f"    📥 服务列表响应:")
        print(f"      - 响应类型: {type(response)}")
        
        if response and response.get('success'):
            services = response.get('data', [])
            print(f"      - 当前服务数量: {len(services)}")
            
            for service in services[:5]:  # 只显示前5个
                name = service.get('name', 'Unknown')
                url = service.get('url', 'Unknown')
                print(f"        * {name}: {url}")
            
            if len(services) > 5:
                print(f"        ... 还有 {len(services) - 5} 个服务")
        else:
            print(f"      - 获取服务列表失败: {response}")
        
        return True
        
    except Exception as e:
        print(f"    ❌ 获取服务列表失败: {e}")
        return False

def main():
    """主测试函数"""
    print("🔧 MCP服务注册调试")
    print("=" * 50)
    
    tests = [
        ("API客户端方法检查", test_api_client_methods),
        ("当前服务列表", test_current_services),
        ("单个添加服务API", test_single_add_service_api),
        ("批量添加服务API", test_batch_add_services_api)
    ]
    
    for test_name, test_func in tests:
        print(f"\n🔬 运行测试: {test_name}")
        try:
            test_func()
        except Exception as e:
            print(f"❌ {test_name} - 异常: {e}")
        
        print("-" * 30)
    
    print("\n💡 调试建议:")
    print("1. 检查API服务器是否正常运行")
    print("2. 检查批量添加API的响应格式")
    print("3. 检查Web界面的错误处理逻辑")
    print("4. 查看浏览器开发者工具的网络请求")

if __name__ == "__main__":
    main()
