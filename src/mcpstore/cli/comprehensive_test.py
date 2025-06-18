#!/usr/bin/env python3
"""
MCPStore Comprehensive Test Suite - 综合测试套件
包含功能测试、API测试、性能测试的完整测试方案
"""
import asyncio
import typer
import time
from typing import Optional

# 导入各个测试模块
from .test_runner import run_tests as run_basic_tests
from .advanced_api_test import run_advanced_api_tests
from .performance_test import run_performance_tests

async def run_comprehensive_tests(
    base_url: str = "http://localhost:18611",
    include_performance: bool = True,
    max_concurrent: int = 10,
    verbose: bool = False
) -> bool:
    """运行综合测试套件"""
    
    typer.echo("🎯 MCPStore Comprehensive Test Suite")
    typer.echo("=" * 70)
    typer.echo(f"Target URL: {base_url}")
    typer.echo(f"Performance Tests: {'Enabled' if include_performance else 'Disabled'}")
    typer.echo(f"Max Concurrent: {max_concurrent}")
    typer.echo(f"Verbose Mode: {'On' if verbose else 'Off'}")
    typer.echo("=" * 70)
    
    start_time = time.time()
    all_passed = True
    
    # 1. 基础功能测试
    typer.echo("\n🔧 Phase 1: Basic Functionality Tests")
    typer.echo("-" * 50)
    try:
        basic_result = await run_basic_tests(
            suite="all",
            host=base_url.split("://")[1].split(":")[0],
            port=int(base_url.split(":")[-1]),
            verbose=verbose
        )
        if basic_result:
            typer.echo("✅ Basic functionality tests: PASSED")
        else:
            typer.echo("❌ Basic functionality tests: FAILED")
            all_passed = False
    except Exception as e:
        typer.echo(f"❌ Basic functionality tests: ERROR - {e}")
        all_passed = False
    
    # 2. 高级API测试
    typer.echo("\n🚀 Phase 2: Advanced API Tests")
    typer.echo("-" * 50)
    try:
        advanced_result = await run_advanced_api_tests(base_url)
        if advanced_result:
            typer.echo("✅ Advanced API tests: PASSED")
        else:
            typer.echo("❌ Advanced API tests: FAILED")
            all_passed = False
    except Exception as e:
        typer.echo(f"❌ Advanced API tests: ERROR - {e}")
        all_passed = False
    
    # 3. 性能测试（可选）
    if include_performance:
        typer.echo("\n⚡ Phase 3: Performance Tests")
        typer.echo("-" * 50)
        try:
            perf_result = await run_performance_tests(base_url, max_concurrent)
            if perf_result:
                typer.echo("✅ Performance tests: PASSED")
            else:
                typer.echo("⚠️  Performance tests: COMPLETED WITH WARNINGS")
                # 性能测试失败不影响整体结果
        except Exception as e:
            typer.echo(f"❌ Performance tests: ERROR - {e}")
            # 性能测试失败不影响整体结果
    
    # 总结
    end_time = time.time()
    total_time = end_time - start_time
    
    typer.echo("\n" + "=" * 70)
    typer.echo("📊 Comprehensive Test Summary")
    typer.echo("=" * 70)
    typer.echo(f"Total Test Time: {total_time:.2f} seconds")
    
    if all_passed:
        typer.echo("🎉 ALL TESTS PASSED! Your MCPStore API is working perfectly!")
        typer.echo("✨ The system is ready for production use.")
    else:
        typer.echo("💥 SOME TESTS FAILED! Please check the issues above.")
        typer.echo("🔧 Fix the problems and run the tests again.")
    
    return all_passed

def create_test_report(results: dict, output_file: str = "mcpstore_test_report.txt"):
    """创建测试报告"""
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write("MCPStore Test Report\n")
        f.write("=" * 50 + "\n")
        f.write(f"Generated at: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        
        for phase, result in results.items():
            f.write(f"{phase}: {'PASSED' if result else 'FAILED'}\n")
        
        f.write("\nDetailed results are available in the console output.\n")
    
    typer.echo(f"📄 Test report saved to: {output_file}")

async def quick_health_check(base_url: str = "http://localhost:18611") -> bool:
    """快速健康检查"""
    import httpx
    
    typer.echo("🏥 Quick Health Check")
    typer.echo("-" * 30)
    
    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            # 检查基本连接
            response = await client.get(f"{base_url}/for_store/health")
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    typer.echo("✅ API Server: Healthy")
                    typer.echo(f"   Status: {data.get('data', {}).get('status', 'unknown')}")
                    return True
                else:
                    typer.echo("⚠️  API Server: Unhealthy")
                    typer.echo(f"   Message: {data.get('message', 'unknown')}")
                    return False
            else:
                typer.echo(f"❌ API Server: HTTP {response.status_code}")
                return False
                
    except Exception as e:
        typer.echo(f"❌ Connection Failed: {e}")
        return False

async def run_smoke_tests(base_url: str = "http://localhost:18611") -> bool:
    """冒烟测试 - 快速验证核心功能"""
    import httpx
    
    typer.echo("💨 Smoke Tests")
    typer.echo("-" * 30)
    
    endpoints = [
        ("/for_store/health", "Health Check"),
        ("/for_store/list_services", "List Services"),
        ("/for_store/list_tools", "List Tools"),
        ("/for_store/get_stats", "Get Statistics"),
    ]
    
    passed = 0
    total = len(endpoints)
    
    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            for endpoint, name in endpoints:
                try:
                    response = await client.get(f"{base_url}{endpoint}")
                    if response.status_code == 200:
                        typer.echo(f"✅ {name}")
                        passed += 1
                    else:
                        typer.echo(f"❌ {name} (HTTP {response.status_code})")
                except Exception as e:
                    typer.echo(f"❌ {name} (Error: {e})")
        
        success_rate = passed / total
        typer.echo(f"\n📊 Smoke Test Results: {passed}/{total} passed ({success_rate*100:.1f}%)")
        
        if success_rate >= 0.8:  # 80%通过率认为是成功
            typer.echo("🎉 Smoke tests passed!")
            return True
        else:
            typer.echo("💥 Smoke tests failed!")
            return False
            
    except Exception as e:
        typer.echo(f"❌ Smoke tests failed: {e}")
        return False

# CLI命令接口
async def main_comprehensive_test(
    base_url: str = "http://localhost:18611",
    test_type: str = "comprehensive",
    performance: bool = True,
    max_concurrent: int = 10,
    verbose: bool = False,
    output_report: Optional[str] = None
):
    """主测试函数"""
    
    if test_type == "health":
        success = await quick_health_check(base_url)
    elif test_type == "smoke":
        success = await run_smoke_tests(base_url)
    elif test_type == "comprehensive":
        success = await run_comprehensive_tests(
            base_url=base_url,
            include_performance=performance,
            max_concurrent=max_concurrent,
            verbose=verbose
        )
    else:
        typer.echo(f"❌ Unknown test type: {test_type}")
        typer.echo("Available types: health, smoke, comprehensive")
        return False
    
    if output_report:
        create_test_report({"test_result": success}, output_report)
    
    return success

if __name__ == "__main__":
    import sys
    
    # 简单的命令行参数解析
    base_url = "http://localhost:18611"
    test_type = "comprehensive"
    
    if len(sys.argv) > 1:
        if sys.argv[1] in ["health", "smoke", "comprehensive"]:
            test_type = sys.argv[1]
        else:
            base_url = sys.argv[1]
    
    if len(sys.argv) > 2:
        if sys.argv[1] in ["health", "smoke", "comprehensive"]:
            base_url = sys.argv[2]
        else:
            test_type = sys.argv[2]
    
    success = asyncio.run(main_comprehensive_test(
        base_url=base_url,
        test_type=test_type
    ))
    
    sys.exit(0 if success else 1)
