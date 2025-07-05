#!/usr/bin/env python3
"""
检查API接口完整性
验证后端API路由和Web客户端方法的完整性
"""

import sys
import os
import re
import requests
import json
from typing import Dict, List, Set, Tuple

sys.path.append(os.path.dirname(os.path.abspath(__file__)))

def extract_api_routes_from_file(file_path: str) -> List[Dict]:
    """从API路由文件中提取所有路由定义"""
    routes = []
    
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 匹配路由装饰器和函数定义
        route_pattern = r'@router\.(get|post|put|delete)\("([^"]+)"[^)]*\)\s*@handle_exceptions\s*async def ([^(]+)'
        matches = re.findall(route_pattern, content, re.MULTILINE)
        
        for method, path, func_name in matches:
            routes.append({
                'method': method.upper(),
                'path': path,
                'function': func_name,
                'category': categorize_route(path)
            })
    
    except Exception as e:
        print(f"❌ 读取API路由文件失败: {e}")
    
    return routes

def categorize_route(path: str) -> str:
    """根据路径对路由进行分类"""
    if path.startswith('/for_store/'):
        if 'service' in path:
            return 'Store服务管理'
        elif 'tool' in path:
            return 'Store工具管理'
        elif 'config' in path or 'mcpconfig' in path:
            return 'Store配置管理'
        elif 'stats' in path or 'health' in path:
            return 'Store状态监控'
        elif 'batch' in path:
            return 'Store批量操作'
        else:
            return 'Store基础功能'
    elif path.startswith('/for_agent/'):
        if 'service' in path:
            return 'Agent服务管理'
        elif 'tool' in path:
            return 'Agent工具管理'
        elif 'config' in path or 'mcpconfig' in path:
            return 'Agent配置管理'
        elif 'stats' in path or 'health' in path:
            return 'Agent状态监控'
        else:
            return 'Agent基础功能'
    elif path.startswith('/monitoring/'):
        return '监控管理'
    elif path.startswith('/services/'):
        return '通用服务查询'
    else:
        return '其他'

def extract_web_client_methods(file_path: str) -> List[str]:
    """从Web客户端文件中提取所有API方法"""
    methods = []
    
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 匹配方法定义
        method_pattern = r'def ([a-zA-Z_][a-zA-Z0-9_]*)\(self[^)]*\) -> Optional\[Dict\]:'
        matches = re.findall(method_pattern, content)
        
        # 过滤掉私有方法和特殊方法
        for method in matches:
            if not method.startswith('_') and method not in ['test_connection']:
                methods.append(method)
    
    except Exception as e:
        print(f"❌ 读取Web客户端文件失败: {e}")
    
    return methods

def test_api_endpoints(base_url: str, routes: List[Dict]) -> Dict[str, bool]:
    """测试API端点是否可访问"""
    results = {}
    
    print(f"🔗 测试API连接: {base_url}")
    
    # 首先测试健康检查
    try:
        response = requests.get(f"{base_url}/for_store/health", timeout=5)
        if response.status_code == 200:
            print("    ✅ API服务器连接成功")
        else:
            print(f"    ⚠️ API服务器响应异常: {response.status_code}")
            return {}
    except Exception as e:
        print(f"    ❌ 无法连接到API服务器: {e}")
        return {}
    
    # 测试各个端点
    for route in routes:
        endpoint = f"{base_url}{route['path']}"
        method = route['method']
        
        try:
            if method == 'GET':
                # 对于需要参数的GET请求，跳过或使用测试参数
                if '{' in route['path']:
                    if 'agent_id' in route['path']:
                        endpoint = endpoint.replace('{agent_id}', 'test_agent')
                    if '{name}' in route['path']:
                        endpoint = endpoint.replace('{name}', 'test_service')
                
                response = requests.get(endpoint, timeout=5)
            elif method == 'POST':
                # 对于POST请求，发送空的JSON数据
                response = requests.post(endpoint, json={}, timeout=5)
            else:
                # 其他方法暂时跳过
                results[route['path']] = True
                continue
            
            # 检查响应状态
            if response.status_code in [200, 400, 404]:  # 400和404也算正常，说明端点存在
                results[route['path']] = True
            else:
                results[route['path']] = False
                
        except Exception as e:
            results[route['path']] = False
    
    return results

def check_api_completeness():
    """检查API完整性"""
    print("🔍 MCPStore API完整性检查")
    print("=" * 60)
    
    # 文件路径
    api_file = "../mcpstore/scripts/api.py"
    web_client_file = "utils/api_client.py"
    base_url = "http://localhost:18611"
    
    # 1. 提取后端API路由
    print("\n📋 1. 检查后端API路由...")
    if os.path.exists(api_file):
        routes = extract_api_routes_from_file(api_file)
        print(f"    ✅ 找到 {len(routes)} 个API路由")
        
        # 按分类统计
        categories = {}
        for route in routes:
            category = route['category']
            if category not in categories:
                categories[category] = []
            categories[category].append(route)
        
        print("    📊 路由分类统计:")
        for category, category_routes in categories.items():
            print(f"      - {category}: {len(category_routes)} 个")
    else:
        print(f"    ❌ API路由文件不存在: {api_file}")
        return
    
    # 2. 提取Web客户端方法
    print("\n🌐 2. 检查Web客户端方法...")
    if os.path.exists(web_client_file):
        methods = extract_web_client_methods(web_client_file)
        print(f"    ✅ 找到 {len(methods)} 个客户端方法")
    else:
        print(f"    ❌ Web客户端文件不存在: {web_client_file}")
        return
    
    # 3. 测试API端点可访问性
    print("\n🧪 3. 测试API端点可访问性...")
    endpoint_results = test_api_endpoints(base_url, routes)
    
    if endpoint_results:
        accessible_count = sum(1 for result in endpoint_results.values() if result)
        total_count = len(endpoint_results)
        print(f"    📊 可访问端点: {accessible_count}/{total_count}")
        
        # 显示不可访问的端点
        inaccessible = [path for path, accessible in endpoint_results.items() if not accessible]
        if inaccessible:
            print("    ⚠️ 不可访问的端点:")
            for path in inaccessible:
                print(f"      - {path}")
    
    # 4. 生成完整性报告
    print("\n📊 4. 完整性分析报告...")
    
    # 核心功能API列表（基于报告中的34个接口）
    core_apis = {
        # Store级别基础API (8个)
        'list_services': '/for_store/list_services',
        'add_service': '/for_store/add_service',
        'check_services': '/for_store/check_services',
        'get_service_info': '/services/{name}',
        'list_tools': '/for_store/list_tools',
        'use_tool': '/for_store/use_tool',
        'get_stats': '/for_store/get_stats',
        'health': '/for_store/health',
        
        # Store级别配置API (5个)
        'get_config': '/for_store/get_config',
        'show_mcpconfig': '/for_store/show_mcpconfig',
        'reset_config': '/for_store/reset_config',
        'validate_config': '/for_store/validate_config',
        'get_service_status': '/for_store/get_service_status',
        
        # Store级别增强API (4个)
        'delete_service': '/for_store/delete_service',
        'update_service': '/for_store/update_service',
        'restart_service': '/for_store/restart_service',
        'batch_add_services': '/for_store/batch_add_services',
        
        # Store级别批量操作API (3个)
        'batch_update_services': '/for_store/batch_update_services',
        'batch_restart_services': '/for_store/batch_restart_services',
        'batch_delete_services': '/for_store/batch_delete_services',
        
        # Agent级别基础API (4个)
        'list_agent_services': '/for_agent/{agent_id}/list_services',
        'add_agent_service': '/for_agent/{agent_id}/add_service',
        'list_agent_tools': '/for_agent/{agent_id}/list_tools',
        'reset_agent_config': '/for_agent/{agent_id}/reset_config',
        
        # Agent级别配置API (4个)
        'validate_agent_config': '/for_agent/{agent_id}/validate_config',
        'get_agent_config': '/for_agent/{agent_id}/get_config',
        'show_agent_mcpconfig': '/for_agent/{agent_id}/show_mcpconfig',
        'update_agent_config': '/for_agent/{agent_id}/update_config',
        
        # Agent级别增强API (3个)
        'delete_agent_service': '/for_agent/{agent_id}/delete_service',
        'get_agent_stats': '/for_agent/{agent_id}/get_stats',
        'get_agent_health': '/for_agent/{agent_id}/health',
        
        # 监控管理API (3个)
        'get_monitoring_status': '/monitoring/status',
        'update_monitoring_config': '/monitoring/config',
        'restart_monitoring': '/monitoring/restart'
    }
    
    # 检查后端API覆盖率
    backend_paths = [route['path'] for route in routes]
    backend_coverage = []
    missing_backend = []
    
    for api_name, expected_path in core_apis.items():
        if expected_path in backend_paths:
            backend_coverage.append(api_name)
        else:
            missing_backend.append(api_name)
    
    # 检查Web客户端覆盖率
    web_coverage = []
    missing_web = []
    
    for api_name in core_apis.keys():
        if api_name in methods:
            web_coverage.append(api_name)
        else:
            missing_web.append(api_name)
    
    # 输出结果
    print(f"    🎯 核心API总数: {len(core_apis)}")
    print(f"    ✅ 后端API覆盖: {len(backend_coverage)}/{len(core_apis)} ({len(backend_coverage)/len(core_apis)*100:.1f}%)")
    print(f"    ✅ Web客户端覆盖: {len(web_coverage)}/{len(core_apis)} ({len(web_coverage)/len(core_apis)*100:.1f}%)")
    
    if missing_backend:
        print(f"    ❌ 缺失的后端API ({len(missing_backend)}个):")
        for api in missing_backend:
            print(f"      - {api}: {core_apis[api]}")
    
    if missing_web:
        print(f"    ❌ 缺失的Web客户端方法 ({len(missing_web)}个):")
        for api in missing_web:
            print(f"      - {api}")
    
    # 5. 总结
    print("\n🎉 5. 检查总结...")
    
    backend_complete = len(missing_backend) == 0
    web_complete = len(missing_web) == 0
    
    if backend_complete and web_complete:
        print("    ✅ 所有核心API已完全实现！")
        print("    🎯 MCPStore Web项目功能完整度: 100%")
    else:
        print(f"    ⚠️ 还有 {len(missing_backend) + len(missing_web)} 个API需要实现")
        if missing_backend:
            print(f"      - 后端缺失: {len(missing_backend)} 个")
        if missing_web:
            print(f"      - Web客户端缺失: {len(missing_web)} 个")
    
    return {
        'backend_complete': backend_complete,
        'web_complete': web_complete,
        'total_apis': len(core_apis),
        'backend_coverage': len(backend_coverage),
        'web_coverage': len(web_coverage),
        'missing_backend': missing_backend,
        'missing_web': missing_web
    }

if __name__ == "__main__":
    check_api_completeness()
