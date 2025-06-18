#!/usr/bin/env python3
"""
MCPStore Performance Test Suite - 性能压力测试
"""
import asyncio
import httpx
import time
import statistics
import typer
from typing import List, Dict, Any
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor

@dataclass
class PerformanceResult:
    endpoint: str
    total_requests: int
    successful_requests: int
    failed_requests: int
    total_time: float
    avg_response_time: float
    min_response_time: float
    max_response_time: float
    requests_per_second: float
    p95_response_time: float
    p99_response_time: float

class PerformanceTester:
    """性能测试器"""
    
    def __init__(self, base_url: str, max_concurrent: int = 10):
        self.base_url = base_url.rstrip('/')
        self.max_concurrent = max_concurrent
        
    async def single_request(self, session: httpx.AsyncClient, endpoint: str, method: str = "GET", data: Dict[str, Any] = None) -> Dict[str, Any]:
        """执行单个请求"""
        start_time = time.time()
        
        try:
            url = f"{self.base_url}{endpoint}"
            
            if method.upper() == "GET":
                response = await session.get(url)
            elif method.upper() == "POST":
                response = await session.post(url, json=data)
            else:
                raise ValueError(f"Unsupported method: {method}")
            
            end_time = time.time()
            
            return {
                "success": True,
                "status_code": response.status_code,
                "response_time": end_time - start_time,
                "response_size": len(response.content)
            }
            
        except Exception as e:
            end_time = time.time()
            return {
                "success": False,
                "error": str(e),
                "response_time": end_time - start_time
            }
    
    async def load_test(self, endpoint: str, num_requests: int, method: str = "GET", data: Dict[str, Any] = None) -> PerformanceResult:
        """负载测试"""
        typer.echo(f"🔥 Load testing {endpoint} with {num_requests} requests...")
        
        # 创建信号量来限制并发数
        semaphore = asyncio.Semaphore(self.max_concurrent)
        
        async def bounded_request(session):
            async with semaphore:
                return await self.single_request(session, endpoint, method, data)
        
        start_time = time.time()
        
        # 创建HTTP客户端
        async with httpx.AsyncClient(timeout=30.0) as session:
            # 执行所有请求
            tasks = [bounded_request(session) for _ in range(num_requests)]
            results = await asyncio.gather(*tasks, return_exceptions=True)
        
        end_time = time.time()
        total_time = end_time - start_time
        
        # 分析结果
        successful_results = []
        failed_results = []
        
        for result in results:
            if isinstance(result, Exception):
                failed_results.append({"error": str(result), "response_time": 0})
            elif result.get("success", False):
                successful_results.append(result)
            else:
                failed_results.append(result)
        
        # 计算统计数据
        if successful_results:
            response_times = [r["response_time"] for r in successful_results]
            avg_response_time = statistics.mean(response_times)
            min_response_time = min(response_times)
            max_response_time = max(response_times)
            
            # 计算百分位数
            sorted_times = sorted(response_times)
            p95_index = int(len(sorted_times) * 0.95)
            p99_index = int(len(sorted_times) * 0.99)
            p95_response_time = sorted_times[p95_index] if p95_index < len(sorted_times) else max_response_time
            p99_response_time = sorted_times[p99_index] if p99_index < len(sorted_times) else max_response_time
        else:
            avg_response_time = min_response_time = max_response_time = 0
            p95_response_time = p99_response_time = 0
        
        requests_per_second = num_requests / total_time if total_time > 0 else 0
        
        return PerformanceResult(
            endpoint=endpoint,
            total_requests=num_requests,
            successful_requests=len(successful_results),
            failed_requests=len(failed_results),
            total_time=total_time,
            avg_response_time=avg_response_time,
            min_response_time=min_response_time,
            max_response_time=max_response_time,
            requests_per_second=requests_per_second,
            p95_response_time=p95_response_time,
            p99_response_time=p99_response_time
        )
    
    async def stress_test(self, endpoint: str, duration_seconds: int, method: str = "GET", data: Dict[str, Any] = None) -> PerformanceResult:
        """压力测试 - 在指定时间内持续发送请求"""
        typer.echo(f"⚡ Stress testing {endpoint} for {duration_seconds} seconds...")
        
        semaphore = asyncio.Semaphore(self.max_concurrent)
        results = []
        start_time = time.time()
        
        async def bounded_request(session):
            async with semaphore:
                return await self.single_request(session, endpoint, method, data)
        
        async with httpx.AsyncClient(timeout=30.0) as session:
            while time.time() - start_time < duration_seconds:
                batch_start = time.time()
                
                # 发送一批请求
                tasks = [bounded_request(session) for _ in range(self.max_concurrent)]
                batch_results = await asyncio.gather(*tasks, return_exceptions=True)
                results.extend(batch_results)
                
                # 控制请求频率，避免过度压力
                batch_time = time.time() - batch_start
                if batch_time < 0.1:  # 最少间隔100ms
                    await asyncio.sleep(0.1 - batch_time)
        
        end_time = time.time()
        total_time = end_time - start_time
        
        # 分析结果（与load_test相同的逻辑）
        successful_results = []
        failed_results = []
        
        for result in results:
            if isinstance(result, Exception):
                failed_results.append({"error": str(result), "response_time": 0})
            elif result.get("success", False):
                successful_results.append(result)
            else:
                failed_results.append(result)
        
        # 计算统计数据
        if successful_results:
            response_times = [r["response_time"] for r in successful_results]
            avg_response_time = statistics.mean(response_times)
            min_response_time = min(response_times)
            max_response_time = max(response_times)
            
            sorted_times = sorted(response_times)
            p95_index = int(len(sorted_times) * 0.95)
            p99_index = int(len(sorted_times) * 0.99)
            p95_response_time = sorted_times[p95_index] if p95_index < len(sorted_times) else max_response_time
            p99_response_time = sorted_times[p99_index] if p99_index < len(sorted_times) else max_response_time
        else:
            avg_response_time = min_response_time = max_response_time = 0
            p95_response_time = p99_response_time = 0
        
        total_requests = len(results)
        requests_per_second = total_requests / total_time if total_time > 0 else 0
        
        return PerformanceResult(
            endpoint=endpoint,
            total_requests=total_requests,
            successful_requests=len(successful_results),
            failed_requests=len(failed_results),
            total_time=total_time,
            avg_response_time=avg_response_time,
            min_response_time=min_response_time,
            max_response_time=max_response_time,
            requests_per_second=requests_per_second,
            p95_response_time=p95_response_time,
            p99_response_time=p99_response_time
        )

def print_performance_result(result: PerformanceResult):
    """打印性能测试结果"""
    typer.echo(f"\n📊 Performance Results for {result.endpoint}")
    typer.echo("─" * 60)
    typer.echo(f"Total Requests:     {result.total_requests}")
    typer.echo(f"Successful:         {result.successful_requests} ({result.successful_requests/result.total_requests*100:.1f}%)")
    typer.echo(f"Failed:             {result.failed_requests} ({result.failed_requests/result.total_requests*100:.1f}%)")
    typer.echo(f"Total Time:         {result.total_time:.3f}s")
    typer.echo(f"Requests/Second:    {result.requests_per_second:.2f}")
    typer.echo(f"Avg Response Time:  {result.avg_response_time*1000:.2f}ms")
    typer.echo(f"Min Response Time:  {result.min_response_time*1000:.2f}ms")
    typer.echo(f"Max Response Time:  {result.max_response_time*1000:.2f}ms")
    typer.echo(f"95th Percentile:    {result.p95_response_time*1000:.2f}ms")
    typer.echo(f"99th Percentile:    {result.p99_response_time*1000:.2f}ms")

async def run_performance_tests(base_url: str = "http://localhost:18611", max_concurrent: int = 10):
    """运行性能测试套件"""
    typer.echo("🏃‍♂️ MCPStore Performance Test Suite")
    typer.echo(f"🎯 Target: {base_url}")
    typer.echo(f"🔀 Max Concurrent: {max_concurrent}")
    typer.echo("─" * 70)
    
    tester = PerformanceTester(base_url, max_concurrent)
    
    # 测试端点列表
    test_endpoints = [
        ("/for_store/health", "GET", None),
        ("/for_store/list_services", "GET", None),
        ("/for_store/list_tools", "GET", None),
        ("/for_store/get_stats", "GET", None),
        ("/for_store/check_services", "GET", None),
    ]
    
    all_results = []
    
    # 负载测试
    typer.echo("🔥 Running Load Tests (100 requests each)...")
    for endpoint, method, data in test_endpoints:
        result = await tester.load_test(endpoint, 100, method, data)
        all_results.append(result)
        print_performance_result(result)
    
    # 压力测试
    typer.echo("\n⚡ Running Stress Tests (30 seconds each)...")
    for endpoint, method, data in test_endpoints[:2]:  # 只测试前两个端点
        result = await tester.stress_test(endpoint, 30, method, data)
        all_results.append(result)
        print_performance_result(result)
    
    # 总结
    typer.echo("\n" + "─" * 70)
    typer.echo("📈 Performance Summary")
    typer.echo("─" * 70)
    
    total_requests = sum(r.total_requests for r in all_results)
    total_successful = sum(r.successful_requests for r in all_results)
    total_failed = sum(r.failed_requests for r in all_results)
    avg_rps = statistics.mean([r.requests_per_second for r in all_results if r.requests_per_second > 0])
    avg_response_time = statistics.mean([r.avg_response_time for r in all_results if r.avg_response_time > 0])
    
    typer.echo(f"Total Requests:        {total_requests}")
    typer.echo(f"Success Rate:          {total_successful/total_requests*100:.1f}%")
    typer.echo(f"Average RPS:           {avg_rps:.2f}")
    typer.echo(f"Average Response Time: {avg_response_time*1000:.2f}ms")
    
    if total_failed == 0:
        typer.echo("🎉 All performance tests passed!")
    else:
        typer.echo(f"⚠️  {total_failed} requests failed")
    
    return total_failed == 0

if __name__ == "__main__":
    import sys
    
    base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:18611"
    max_concurrent = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    
    success = asyncio.run(run_performance_tests(base_url, max_concurrent))
    sys.exit(0 if success else 1)
