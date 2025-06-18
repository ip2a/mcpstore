#!/usr/bin/env python3
"""
MCPStore Advanced API Test Suite - 高级API功能测试
"""
import asyncio
import httpx
import json
import time
import typer
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

@dataclass
class APITestCase:
    name: str
    method: str
    url: str
    data: Optional[Dict[str, Any]] = None
    expected_status: int = 200
    description: str = ""

class AdvancedAPITester:
    """高级API测试器"""
    
    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip('/')
        self.client = httpx.AsyncClient(timeout=30.0)
        
    async def __aenter__(self):
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.client.aclose()
    
    async def run_test_case(self, test_case: APITestCase) -> Dict[str, Any]:
        """运行单个测试用例"""
        start_time = time.time()
        
        try:
            url = f"{self.base_url}{test_case.url}"
            
            if test_case.method.upper() == "GET":
                response = await self.client.get(url)
            elif test_case.method.upper() == "POST":
                response = await self.client.post(url, json=test_case.data)
            elif test_case.method.upper() == "PUT":
                response = await self.client.put(url, json=test_case.data)
            elif test_case.method.upper() == "DELETE":
                response = await self.client.delete(url)
            else:
                raise ValueError(f"Unsupported method: {test_case.method}")
            
            duration = time.time() - start_time
            
            # 解析响应
            try:
                response_data = response.json()
            except:
                response_data = {"raw": response.text}
            
            success = response.status_code == test_case.expected_status
            
            return {
                "name": test_case.name,
                "success": success,
                "status_code": response.status_code,
                "expected_status": test_case.expected_status,
                "duration": duration,
                "response_data": response_data,
                "description": test_case.description
            }
            
        except Exception as e:
            duration = time.time() - start_time
            return {
                "name": test_case.name,
                "success": False,
                "error": str(e),
                "duration": duration,
                "description": test_case.description
            }

def get_store_test_cases() -> List[APITestCase]:
    """获取Store级别测试用例"""
    return [
        # 基础查询测试
        APITestCase(
            name="Store Health Check",
            method="GET",
            url="/for_store/health",
            description="检查Store级别系统健康状态"
        ),
        APITestCase(
            name="Store List Services",
            method="GET",
            url="/for_store/list_services",
            description="获取Store级别服务列表"
        ),
        APITestCase(
            name="Store List Tools",
            method="GET",
            url="/for_store/list_tools",
            description="获取Store级别工具列表"
        ),
        APITestCase(
            name="Store Check Services",
            method="GET",
            url="/for_store/check_services",
            description="检查Store级别服务健康状态"
        ),
        APITestCase(
            name="Store Get Stats",
            method="GET",
            url="/for_store/get_stats",
            description="获取Store级别统计信息"
        ),
        APITestCase(
            name="Store Get Config",
            method="GET",
            url="/for_store/get_config",
            description="获取Store级别配置"
        ),
        APITestCase(
            name="Store Validate Config",
            method="GET",
            url="/for_store/validate_config",
            description="验证Store级别配置"
        ),

        # 服务添加测试 - 空参数注册所有服务
        APITestCase(
            name="Store Add Service (All)",
            method="POST",
            url="/for_store/add_service",
            data=None,  # 空参数，注册mcp.json中的所有服务
            description="注册mcp.json中的所有服务"
        ),

        # 服务添加测试 - 单个服务配置（不带mcpServers字段）
        APITestCase(
            name="Store Add Service (高德)",
            method="POST",
            url="/for_store/add_service",
            data={
                "name": "测试高德服务",
                "url": "https://mcp.amap.com/sse?key=da2c9c39f9edad643b9c53f506fb381c",
                "transport": "sse"
            },
            description="添加高德地图服务（单个服务配置格式）"
        ),
        # 服务添加测试 - 带mcpServers字段的配置
        APITestCase(
            name="Store Add Service (mcpServers格式)",
            method="POST",
            url="/for_store/add_service",
            data={
                "mcpServers": {
                    "测试天气服务": {
                        "url": "http://127.0.0.1:8000/mcp"
                    }
                }
            },
            description="添加天气服务（mcpServers配置格式）"
        ),

        # 服务添加测试 - 缺少transport字段（预期失败）
        APITestCase(
            name="Store Add Service (天气)",
            method="POST",
            url="/for_store/add_service",
            data={
                "name": "test_weather_fail",
                "url": "http://127.0.0.1:8000/mcp"
            },
            expected_status=400,  # 缺少transport字段
            description="添加天气服务（预期失败 - 缺少transport）"
        ),

        # 服务信息查询测试
        APITestCase(
            name="Store Get Service Info (Nonexistent)",
            method="POST",
            url="/for_store/get_service_info",
            data={"name": "nonexistent_service"},
            expected_status=404,
            description="获取不存在服务的信息（预期失败）"
        ),
        APITestCase(
            name="Store Get Service Status (Nonexistent)",
            method="POST",
            url="/for_store/get_service_status",
            data={"name": "nonexistent_service"},
            expected_status=404,
            description="获取不存在服务的状态（预期失败）"
        ),

        # 批量操作测试
        APITestCase(
            name="Store Batch Add Services (Empty)",
            method="POST",
            url="/for_store/batch_add_services",
            data={"services": []},
            expected_status=400,
            description="批量添加空服务列表（预期失败）"
        ),
        APITestCase(
            name="Store Batch Update Services (Empty)",
            method="POST",
            url="/for_store/batch_update_services",
            data={"updates": []},
            expected_status=400,
            description="批量更新空服务列表（预期失败）"
        ),
        APITestCase(
            name="Store Batch Add Services (Valid)",
            method="POST",
            url="/for_store/batch_add_services",
            data={
                "services": [
                    {
                        "name": "batch_gaode",
                        "url": "https://mcp.amap.com/sse?key=da2c9c39f9edad643b9c53f506fb381c",
                        "transport": "sse"
                    },
                    {
                        "name": "batch_weather",
                        "url": "http://127.0.0.1:8000/mcp"
                    }
                ]
            },
            description="批量添加有效服务"
        ),

        # 重置配置测试
        APITestCase(
            name="Store Reset Config",
            method="POST",
            url="/for_store/reset_config",
            description="Store级别重置配置"
        ),
        APITestCase(
            name="Store Reset JSON Config",
            method="POST",
            url="/for_store/reset_json_config",
            description="Store级别重置JSON配置文件"
        ),
        APITestCase(
            name="Store Restore Default Config",
            method="POST",
            url="/for_store/restore_default_config",
            description="Store级别恢复默认配置"
        ),
    ]

def get_agent_test_cases() -> List[APITestCase]:
    """获取Agent级别测试用例"""
    agent_id = "test_agent_advanced"

    return [
        # 基础查询测试
        APITestCase(
            name="Agent Health Check",
            method="GET",
            url=f"/for_agent/{agent_id}/health",
            description=f"检查Agent {agent_id} 健康状态"
        ),
        APITestCase(
            name="Agent List Services",
            method="GET",
            url=f"/for_agent/{agent_id}/list_services",
            description=f"获取Agent {agent_id} 服务列表"
        ),
        APITestCase(
            name="Agent List Tools",
            method="GET",
            url=f"/for_agent/{agent_id}/list_tools",
            description=f"获取Agent {agent_id} 工具列表"
        ),
        APITestCase(
            name="Agent Check Services",
            method="GET",
            url=f"/for_agent/{agent_id}/check_services",
            description=f"检查Agent {agent_id} 服务健康状态"
        ),
        APITestCase(
            name="Agent Get Stats",
            method="GET",
            url=f"/for_agent/{agent_id}/get_stats",
            description=f"获取Agent {agent_id} 统计信息"
        ),
        APITestCase(
            name="Agent Get Config",
            method="GET",
            url=f"/for_agent/{agent_id}/get_config",
            description=f"获取Agent {agent_id} 配置"
        ),
        APITestCase(
            name="Agent Validate Config",
            method="GET",
            url=f"/for_agent/{agent_id}/validate_config",
            description=f"验证Agent {agent_id} 配置"
        ),

        # Agent服务添加测试 - 通过名称列表添加已存在的服务
        APITestCase(
            name="Agent Add Service (By Name)",
            method="POST",
            url=f"/for_agent/{agent_id}/add_service",
            data=["高德", "天气服务"],  # 添加已存在的服务
            description=f"Agent {agent_id} 通过名称添加服务"
        ),

        # Agent服务添加测试 - 通过单个服务配置添加
        APITestCase(
            name="Agent Add Service (Single Config)",
            method="POST",
            url=f"/for_agent/{agent_id}/add_service",
            data={
                "name": "Agent新增服务",
                "url": "http://127.0.0.1:8000/mcp",
                "transport": "streamable-http"
            },
            description=f"Agent {agent_id} 通过单个服务配置添加新服务"
        ),

        # Agent服务添加测试 - 通过mcpServers配置添加
        APITestCase(
            name="Agent Add Service (By Config)",
            method="POST",
            url=f"/for_agent/{agent_id}/add_service",
            data={
                "mcpServers": {
                    "agent_test_weather": {
                        "url": "http://127.0.0.1:8000/mcp"
                    }
                }
            },
            description=f"Agent {agent_id} 通过mcpServers配置添加服务"
        ),

        # 服务信息查询测试
        APITestCase(
            name="Agent Get Service Info (Nonexistent)",
            method="POST",
            url=f"/for_agent/{agent_id}/get_service_info",
            data={"name": "nonexistent_service"},
            expected_status=404,
            description=f"获取Agent {agent_id} 不存在服务的信息（预期失败）"
        ),
        APITestCase(
            name="Agent Get Service Status (Nonexistent)",
            method="POST",
            url=f"/for_agent/{agent_id}/get_service_status",
            data={"name": "nonexistent_service"},
            expected_status=404,
            description=f"获取Agent {agent_id} 不存在服务的状态（预期失败）"
        ),

        # 批量操作测试
        APITestCase(
            name="Agent Batch Add Services (Empty)",
            method="POST",
            url=f"/for_agent/{agent_id}/batch_add_services",
            data={"services": []},
            expected_status=400,
            description=f"Agent {agent_id} 批量添加空服务列表（预期失败）"
        ),
        APITestCase(
            name="Agent Batch Update Services (Empty)",
            method="POST",
            url=f"/for_agent/{agent_id}/batch_update_services",
            data={"updates": []},
            expected_status=400,
            description=f"Agent {agent_id} 批量更新空服务列表（预期失败）"
        ),
        APITestCase(
            name="Agent Batch Add Services (Valid)",
            method="POST",
            url=f"/for_agent/{agent_id}/batch_add_services",
            data={
                "services": [
                    "高德",  # 通过名称添加
                    {
                        "name": "agent_batch_weather",
                        "url": "http://127.0.0.1:8000/mcp"
                    }
                ]
            },
            description=f"Agent {agent_id} 批量添加有效服务"
        ),

        # Agent重置配置测试
        APITestCase(
            name="Agent Reset Config",
            method="POST",
            url=f"/for_agent/{agent_id}/reset_config",
            description=f"Agent {agent_id} 重置配置"
        ),
    ]

def get_tool_usage_test_cases() -> List[APITestCase]:
    """获取工具使用测试用例"""
    agent_id = "test_agent_tools"

    return [
        # Store级别工具使用（需要先添加服务）
        APITestCase(
            name="Store Use Tool (Map Direction)",
            method="POST",
            url="/for_store/use_tool",
            data={
                "tool_name": "gaode_maps_direction_driving",
                "args": {
                    "origin": "116.481028,39.989643",
                    "destination": "116.434446,39.90816"
                }
            },
            description="Store级别使用高德地图导航工具"
        ),
        APITestCase(
            name="Store Use Tool (Weather)",
            method="POST",
            url="/for_store/use_tool",
            data={
                "tool_name": "get_weather",
                "args": {
                    "location": "北京"
                }
            },
            description="Store级别使用天气查询工具"
        ),
        APITestCase(
            name="Store Use Tool (Nonexistent)",
            method="POST",
            url="/for_store/use_tool",
            data={
                "tool_name": "nonexistent_tool",
                "args": {}
            },
            expected_status=400,
            description="Store级别使用不存在的工具（预期失败）"
        ),

        # Agent级别工具使用
        APITestCase(
            name="Agent Use Tool (Map Walking)",
            method="POST",
            url=f"/for_agent/{agent_id}/use_tool",
            data={
                "tool_name": "gaode_maps_direction_walking",
                "args": {
                    "origin": "116.481028,39.989643",
                    "destination": "116.434446,39.90816"
                }
            },
            description=f"Agent {agent_id} 使用高德地图步行导航工具"
        ),
        APITestCase(
            name="Agent Use Tool (Weather Forecast)",
            method="POST",
            url=f"/for_agent/{agent_id}/use_tool",
            data={
                "tool_name": "get_weather_forecast",
                "args": {
                    "location": "上海",
                    "days": 3
                }
            },
            description=f"Agent {agent_id} 使用天气预报工具"
        ),
        APITestCase(
            name="Agent Use Tool (Nonexistent)",
            method="POST",
            url=f"/for_agent/{agent_id}/use_tool",
            data={
                "tool_name": "nonexistent_tool",
                "args": {}
            },
            expected_status=400,
            description=f"Agent {agent_id} 使用不存在的工具（预期失败）"
        ),
    ]

def get_service_management_test_cases() -> List[APITestCase]:
    """获取服务管理测试用例"""
    agent_id = "test_agent_mgmt"

    return [
        # Store级别服务管理
        APITestCase(
            name="Store Delete Service",
            method="POST",
            url="/for_store/delete_service",
            data={"name": "test_gaode"},
            description="Store级别删除服务"
        ),
        APITestCase(
            name="Store Update Service",
            method="POST",
            url="/for_store/update_service",
            data={
                "name": "test_weather",
                "config": {
                    "url": "http://127.0.0.1:8000/mcp",
                    "description": "Updated weather service"
                }
            },
            description="Store级别更新服务配置"
        ),
        APITestCase(
            name="Store Restart Service",
            method="POST",
            url="/for_store/restart_service",
            data={"name": "test_weather"},
            description="Store级别重启服务"
        ),

        # Agent级别服务管理
        APITestCase(
            name="Agent Delete Service",
            method="POST",
            url=f"/for_agent/{agent_id}/delete_service",
            data={"name": "高德"},
            description=f"Agent {agent_id} 删除服务"
        ),
        APITestCase(
            name="Agent Update Service",
            method="POST",
            url=f"/for_agent/{agent_id}/update_service",
            data={
                "name": "agent_test_weather",
                "config": {
                    "url": "http://127.0.0.1:8000/mcp",
                    "description": "Updated agent weather service"
                }
            },
            description=f"Agent {agent_id} 更新服务配置"
        ),
        APITestCase(
            name="Agent Restart Service",
            method="POST",
            url=f"/for_agent/{agent_id}/restart_service",
            data={"name": "agent_test_weather"},
            description=f"Agent {agent_id} 重启服务"
        ),
    ]

def get_error_handling_test_cases() -> List[APITestCase]:
    """获取错误处理测试用例"""
    return [
        # 无效路径测试
        APITestCase(
            name="Invalid Endpoint",
            method="GET",
            url="/invalid/endpoint",
            expected_status=404,
            description="访问不存在的端点（预期404）"
        ),

        # 无效Agent ID测试
        APITestCase(
            name="Invalid Agent ID",
            method="GET",
            url="/for_agent/invalid@agent/list_services",
            expected_status=400,
            description="使用无效Agent ID（预期400）"
        ),

        # 缺少必需参数测试
        APITestCase(
            name="Missing Service Name",
            method="POST",
            url="/for_store/get_service_info",
            data={},
            expected_status=400,
            description="缺少服务名称参数（预期400）"
        ),

        # 无效JSON测试
        APITestCase(
            name="Invalid Request Data",
            method="POST",
            url="/for_store/delete_service",
            data={"invalid": "data"},
            expected_status=400,
            description="发送无效请求数据（预期400）"
        ),

        # 无效工具参数测试
        APITestCase(
            name="Invalid Tool Args",
            method="POST",
            url="/for_store/use_tool",
            data={
                "tool_name": "map_maps_direction_driving",
                "args": "invalid_args"  # 应该是字典
            },
            expected_status=400,
            description="使用无效工具参数（预期400）"
        ),

        # 缺少工具名称测试
        APITestCase(
            name="Missing Tool Name",
            method="POST",
            url="/for_store/use_tool",
            data={
                "args": {"test": "value"}
            },
            expected_status=400,
            description="缺少工具名称（预期400）"
        ),
    ]

def get_config_sync_test_cases() -> List[APITestCase]:
    """获取配置文件同步验证测试用例"""
    return [
        # 配置文件查看测试
        APITestCase(
            name="Store Show MCP Config",
            method="GET",
            url="/for_store/show_mcpconfig",
            description="查看Store级别的MCP配置"
        ),
        APITestCase(
            name="Agent Show MCP Config",
            method="GET",
            url="/for_agent/test_agent_config/show_mcpconfig",
            description="查看Agent级别的MCP配置"
        ),

        # 配置验证测试
        APITestCase(
            name="Store Validate Config",
            method="GET",
            url="/for_store/validate_config",
            description="验证Store级别配置完整性"
        ),
        APITestCase(
            name="Agent Validate Config",
            method="GET",
            url="/for_agent/test_agent_config/validate_config",
            description="验证Agent级别配置完整性"
        ),
    ]

async def run_advanced_api_tests(base_url: str = "http://localhost:18611"):
    """运行高级API测试"""
    typer.echo("🚀 MCPStore Advanced API Test Suite")
    typer.echo(f"🎯 Target: {base_url}")
    typer.echo("─" * 70)

    async with AdvancedAPITester(base_url) as tester:
        all_test_cases = []

        # 收集所有测试用例
        typer.echo("📋 Collecting test cases...")
        store_cases = get_store_test_cases()
        agent_cases = get_agent_test_cases()
        tool_cases = get_tool_usage_test_cases()
        mgmt_cases = get_service_management_test_cases()
        config_cases = get_config_sync_test_cases()
        error_cases = get_error_handling_test_cases()

        all_test_cases.extend(store_cases)
        all_test_cases.extend(agent_cases)
        all_test_cases.extend(tool_cases)
        all_test_cases.extend(mgmt_cases)
        all_test_cases.extend(config_cases)
        all_test_cases.extend(error_cases)

        typer.echo(f"   Store tests: {len(store_cases)}")
        typer.echo(f"   Agent tests: {len(agent_cases)}")
        typer.echo(f"   Tool usage tests: {len(tool_cases)}")
        typer.echo(f"   Service management tests: {len(mgmt_cases)}")
        typer.echo(f"   Config sync tests: {len(config_cases)}")
        typer.echo(f"   Error handling tests: {len(error_cases)}")
        typer.echo(f"   Total: {len(all_test_cases)} tests")
        typer.echo()

        results = []

        # 运行测试
        for i, test_case in enumerate(all_test_cases, 1):
            typer.echo(f"[{i:3d}/{len(all_test_cases)}] {test_case.name}")
            result = await tester.run_test_case(test_case)
            results.append(result)

            # 显示结果
            status = "✅" if result["success"] else "❌"
            duration = result.get("duration", 0)
            typer.echo(f"         {status} {duration:.3f}s - {test_case.description}")

            if not result["success"] and "error" in result:
                typer.echo(f"            Error: {result['error']}")
            elif not result["success"]:
                expected = result.get("expected_status", "unknown")
                actual = result.get("status_code", "unknown")
                typer.echo(f"            Expected: {expected}, Got: {actual}")

        # 统计结果
        typer.echo("\n" + "─" * 70)
        passed = sum(1 for r in results if r["success"])
        failed = len(results) - passed
        total_time = sum(r.get("duration", 0) for r in results)

        # 按类别统计
        idx = 0
        store_passed = sum(1 for r in results[idx:idx+len(store_cases)] if r["success"])
        idx += len(store_cases)
        agent_passed = sum(1 for r in results[idx:idx+len(agent_cases)] if r["success"])
        idx += len(agent_cases)
        tool_passed = sum(1 for r in results[idx:idx+len(tool_cases)] if r["success"])
        idx += len(tool_cases)
        mgmt_passed = sum(1 for r in results[idx:idx+len(mgmt_cases)] if r["success"])
        idx += len(mgmt_cases)
        config_passed = sum(1 for r in results[idx:idx+len(config_cases)] if r["success"])
        idx += len(config_cases)
        error_passed = sum(1 for r in results[idx:idx+len(error_cases)] if r["success"])

        typer.echo("📊 Results by Category:")
        typer.echo(f"   Store tests:        {store_passed}/{len(store_cases)} passed")
        typer.echo(f"   Agent tests:        {agent_passed}/{len(agent_cases)} passed")
        typer.echo(f"   Tool usage tests:   {tool_passed}/{len(tool_cases)} passed")
        typer.echo(f"   Service mgmt tests: {mgmt_passed}/{len(mgmt_cases)} passed")
        typer.echo(f"   Config sync tests:  {config_passed}/{len(config_cases)} passed")
        typer.echo(f"   Error handling:     {error_passed}/{len(error_cases)} passed")
        typer.echo(f"   Overall:            {passed}/{len(results)} passed")
        typer.echo(f"⏱️  Total time: {total_time:.3f}s")

        if failed == 0:
            typer.echo("🎉 All tests passed!")
        else:
            typer.echo(f"💥 {failed} test(s) failed!")

        return failed == 0

if __name__ == "__main__":
    import sys
    
    base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:18611"
    success = asyncio.run(run_advanced_api_tests(base_url))
    sys.exit(0 if success else 1)
