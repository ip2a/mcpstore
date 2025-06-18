#!/usr/bin/env python3
"""
MCPStore Test Runner - 精巧的API测试套件
"""
import asyncio
import httpx
import json
import time
import typer
from typing import Dict, List, Any, Optional
from dataclasses import dataclass
from enum import Enum

class TestStatus(Enum):
    PASS = "✅"
    FAIL = "❌"
    SKIP = "⏭️"
    WARN = "⚠️"

@dataclass
class TestResult:
    name: str
    status: TestStatus
    message: str
    duration: float
    details: Optional[Dict[str, Any]] = None

class MCPStoreAPITester:
    """MCPStore API测试器"""
    
    def __init__(self, base_url: str, verbose: bool = False):
        self.base_url = base_url.rstrip('/')
        self.verbose = verbose
        self.client = httpx.AsyncClient(timeout=30.0)
        self.results: List[TestResult] = []
        
    async def __aenter__(self):
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.client.aclose()
    
    def log(self, message: str, level: str = "info"):
        """日志输出"""
        if self.verbose or level == "error":
            timestamp = time.strftime("%H:%M:%S")
            typer.echo(f"[{timestamp}] {message}")
    
    async def test_health_check(self) -> TestResult:
        """测试健康检查"""
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_store/health")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return TestResult(
                        name="Health Check",
                        status=TestStatus.PASS,
                        message="API server is healthy",
                        duration=duration,
                        details=data.get("data")
                    )
                else:
                    return TestResult(
                        name="Health Check",
                        status=TestStatus.WARN,
                        message=f"API unhealthy: {data.get('message')}",
                        duration=duration
                    )
            else:
                return TestResult(
                    name="Health Check",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                )
        except Exception as e:
            duration = time.time() - start_time
            return TestResult(
                name="Health Check",
                status=TestStatus.FAIL,
                message=f"Connection failed: {str(e)}",
                duration=duration
            )
    
    async def test_store_operations(self) -> List[TestResult]:
        """测试Store级别操作"""
        results = []
        
        # 测试获取服务列表
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_store/list_services")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Store List Services",
                    status=TestStatus.PASS,
                    message=f"Found {len(data.get('data', []))} services",
                    duration=duration,
                    details={"service_count": len(data.get('data', []))}
                ))
            else:
                results.append(TestResult(
                    name="Store List Services",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Store List Services",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        # 测试获取工具列表
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_store/list_tools")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Store List Tools",
                    status=TestStatus.PASS,
                    message=f"Found {len(data.get('data', []))} tools",
                    duration=duration,
                    details={"tool_count": len(data.get('data', []))}
                ))
            else:
                results.append(TestResult(
                    name="Store List Tools",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Store List Tools",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        # 测试健康检查
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_store/check_services")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Store Check Services",
                    status=TestStatus.PASS,
                    message="Health check completed",
                    duration=duration,
                    details=data.get('data')
                ))
            else:
                results.append(TestResult(
                    name="Store Check Services",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Store Check Services",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        # 测试获取统计信息
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_store/get_stats")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                stats = data.get('data', {})
                results.append(TestResult(
                    name="Store Get Stats",
                    status=TestStatus.PASS,
                    message=f"Stats retrieved: {stats.get('services', {}).get('total', 0)} services",
                    duration=duration,
                    details=stats
                ))
            else:
                results.append(TestResult(
                    name="Store Get Stats",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Store Get Stats",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        return results
    
    async def test_agent_operations(self) -> List[TestResult]:
        """测试Agent级别操作"""
        results = []
        agent_id = "test_agent_123"
        
        # 测试Agent服务列表
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_agent/{agent_id}/list_services")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Agent List Services",
                    status=TestStatus.PASS,
                    message=f"Agent {agent_id}: {len(data.get('data', []))} services",
                    duration=duration,
                    details={"agent_id": agent_id, "service_count": len(data.get('data', []))}
                ))
            else:
                results.append(TestResult(
                    name="Agent List Services",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Agent List Services",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        # 测试Agent工具列表
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_agent/{agent_id}/list_tools")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Agent List Tools",
                    status=TestStatus.PASS,
                    message=f"Agent {agent_id}: {len(data.get('data', []))} tools",
                    duration=duration,
                    details={"agent_id": agent_id, "tool_count": len(data.get('data', []))}
                ))
            else:
                results.append(TestResult(
                    name="Agent List Tools",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Agent List Tools",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        # 测试Agent健康检查
        start_time = time.time()
        try:
            response = await self.client.get(f"{self.base_url}/for_agent/{agent_id}/health")
            duration = time.time() - start_time
            
            if response.status_code == 200:
                data = response.json()
                results.append(TestResult(
                    name="Agent Health Check",
                    status=TestStatus.PASS,
                    message=f"Agent {agent_id} health check completed",
                    duration=duration,
                    details=data.get('data')
                ))
            else:
                results.append(TestResult(
                    name="Agent Health Check",
                    status=TestStatus.FAIL,
                    message=f"HTTP {response.status_code}",
                    duration=duration
                ))
        except Exception as e:
            duration = time.time() - start_time
            results.append(TestResult(
                name="Agent Health Check",
                status=TestStatus.FAIL,
                message=f"Request failed: {str(e)}",
                duration=duration
            ))
        
        return results

async def run_tests(suite: str = "all", host: str = "localhost", port: int = 18611, verbose: bool = False) -> bool:
    """运行测试套件"""
    base_url = f"http://{host}:{port}"

    # 支持不同的测试套件
    if suite == "comprehensive":
        from .comprehensive_test import run_comprehensive_tests
        return await run_comprehensive_tests(base_url, verbose=verbose)
    elif suite == "advanced":
        from .advanced_api_test import run_advanced_api_tests
        return await run_advanced_api_tests(base_url)
    elif suite == "performance":
        from .performance_test import run_performance_tests
        return await run_performance_tests(base_url)
    elif suite == "smoke":
        from .comprehensive_test import run_smoke_tests
        return await run_smoke_tests(base_url)
    elif suite == "health":
        from .comprehensive_test import quick_health_check
        return await quick_health_check(base_url)

    # 默认基础测试套件
    typer.echo("🧪 MCPStore Basic API Test Suite")
    typer.echo(f"🎯 Target: {base_url}")
    typer.echo("─" * 50)

    async with MCPStoreAPITester(base_url, verbose) as tester:
        all_results = []

        # 首先测试连接
        health_result = await tester.test_health_check()
        all_results.append(health_result)

        if health_result.status == TestStatus.FAIL:
            typer.echo(f"{health_result.status.value} {health_result.name}: {health_result.message}")
            typer.echo("❌ Cannot connect to API server. Please ensure it's running.")
            return False

        # 根据套件运行测试
        if suite in ["all", "api", "core"]:
            # Store级别测试
            store_results = await tester.test_store_operations()
            all_results.extend(store_results)

            # Agent级别测试
            agent_results = await tester.test_agent_operations()
            all_results.extend(agent_results)

        # 显示结果
        typer.echo("\n📊 Test Results:")
        typer.echo("─" * 50)

        passed = 0
        failed = 0
        warnings = 0

        for result in all_results:
            status_icon = result.status.value
            duration_str = f"{result.duration:.3f}s"
            typer.echo(f"{status_icon} {result.name:<25} {duration_str:>8} - {result.message}")

            if result.status == TestStatus.PASS:
                passed += 1
            elif result.status == TestStatus.FAIL:
                failed += 1
            elif result.status == TestStatus.WARN:
                warnings += 1

        # 总结
        typer.echo("─" * 50)
        total = len(all_results)
        typer.echo(f"📈 Summary: {passed} passed, {failed} failed, {warnings} warnings ({total} total)")

        if failed == 0:
            typer.echo("🎉 All tests passed!")
            return True
        else:
            typer.echo(f"💥 {failed} test(s) failed!")
            return False
