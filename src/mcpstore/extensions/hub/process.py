"""
Hub Process Module
Hub进程模块 - 管理Hub服务器进程的生命周期
"""

import asyncio
import logging
import os
import subprocess
import time
from datetime import datetime
from typing import List, Dict, Any, Optional

from .types import HubStatus, HubProcessInfo, HubServiceInfo, HubRouteInfo

logger = logging.getLogger(__name__)


class HubProcess:
    """
    Hub进程管理器
    
    负责管理单个Hub服务器进程的完整生命周期，包括：
    - 进程启动和停止
    - 状态监控和健康检查
    - 资源清理和错误处理
    - 路由信息管理
    
    特点：
    - 完整的进程生命周期管理
    - 优雅的启动和关闭机制
    - 自动资源清理
    - 详细的状态报告
    """
    
    def __init__(
        self, 
        package_name: str, 
        process: subprocess.Popen, 
        port: int,
        config_file: str, 
        script_file: str,
        services: List[HubServiceInfo]
    ):
        """
        初始化Hub进程管理器
        
        Args:
            package_name: Hub包名
            process: 子进程对象
            port: 服务端口
            config_file: 配置文件路径
            script_file: 脚本文件路径
            services: 服务列表
        """
        self.package_name = package_name
        self.process = process
        self.port = port
        self.config_file = config_file
        self.script_file = script_file
        self.services = services
        
        # 进程状态
        self.start_time = datetime.now()
        self._status = HubStatus.INITIALIZING
        self._startup_timeout = 30  # 启动超时时间（秒）
        
        logger.debug(f"HubProcess '{package_name}' initialized with PID {process.pid}")
    
    @property
    def is_running(self) -> bool:
        """
        检查进程是否正在运行
        
        Returns:
            bool: 进程是否存活
        """
        if self.process is None:
            return False
        
        return self.process.poll() is None
    
    @property
    def status(self) -> HubStatus:
        """
        获取Hub状态
        
        Returns:
            HubStatus: 当前状态
        """
        if not self.is_running and self._status != HubStatus.STOPPED:
            self._status = HubStatus.ERROR
        return self._status
    
    @property
    def endpoint_url(self) -> str:
        """
        获取服务端点URL
        
        Returns:
            str: MCP服务端点URL
        """
        return f"http://localhost:{self.port}/mcp"
    
    @property
    def uptime(self) -> float:
        """
        获取运行时长
        
        Returns:
            float: 运行时长（秒）
        """
        if not self.is_running:
            return 0.0
        return (datetime.now() - self.start_time).total_seconds()
    
    async def wait_for_startup(self, timeout: Optional[float] = None) -> bool:
        """
        等待Hub服务器启动完成
        
        Args:
            timeout: 超时时间，None使用默认值
            
        Returns:
            bool: 是否启动成功
        """
        if timeout is None:
            timeout = self._startup_timeout
        
        logger.debug(f"Waiting for Hub '{self.package_name}' to start (timeout: {timeout}s)")
        
        start_time = time.time()
        while time.time() - start_time < timeout:
            if not self.is_running:
                logger.error(f"Hub process '{self.package_name}' terminated during startup")
                self._status = HubStatus.ERROR
                return False
            
            # 检查服务器是否响应
            if await self._check_server_health():
                logger.info(f"Hub '{self.package_name}' started successfully")
                self._status = HubStatus.RUNNING
                return True
            
            await asyncio.sleep(1)
        
        logger.warning(f"Hub '{self.package_name}' startup timeout after {timeout}s")
        self._status = HubStatus.ERROR
        return False
    
    async def _check_server_health(self) -> bool:
        """
        检查服务器健康状态

        通过HTTP请求检查MCP服务器是否正常响应。

        Returns:
            bool: 服务器是否健康
        """
        try:
            import httpx

            # 简单的健康检查：尝试连接MCP端点
            async with httpx.AsyncClient() as client:
                response = await client.get(
                    self.endpoint_url,
                    timeout=5.0
                )
                # MCP服务器应该返回200或405（GET方法可能不被支持）
                return response.status_code in [200, 405]

        except Exception as e:
            logger.debug(f"Health check failed for '{self.package_name}': {e}")
            return False
    
    async def stop_async(self, force: bool = False, timeout: float = 10.0) -> bool:
        """
        异步停止Hub服务器
        
        Args:
            force: 是否强制终止
            timeout: 优雅停止的超时时间
            
        Returns:
            bool: 是否成功停止
        """
        if not self.is_running:
            logger.debug(f"Hub '{self.package_name}' is already stopped")
            return True
        
        logger.info(f"Stopping Hub '{self.package_name}' (PID: {self.process.pid})")
        self._status = HubStatus.STOPPING
        
        try:
            if force:
                # 强制终止
                self.process.kill()
                logger.info(f"Force killed Hub '{self.package_name}'")
            else:
                # 优雅停止
                self.process.terminate()
                
                # 等待进程优雅退出
                try:
                    await asyncio.wait_for(
                        self._wait_for_process_exit(),
                        timeout=timeout
                    )
                    logger.info(f"Hub '{self.package_name}' stopped gracefully")
                except asyncio.TimeoutError:
                    logger.warning(f"Hub '{self.package_name}' did not stop gracefully, killing")
                    self.process.kill()
                    await self._wait_for_process_exit()
            
            # 清理资源
            await self._cleanup_resources()
            self._status = HubStatus.STOPPED
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to stop Hub '{self.package_name}': {e}")
            self._status = HubStatus.ERROR
            return False
    
    def stop(self, force: bool = False, timeout: float = 10.0) -> bool:
        """
        同步停止Hub服务器
        
        Args:
            force: 是否强制终止
            timeout: 优雅停止的超时时间
            
        Returns:
            bool: 是否成功停止
        """
        return asyncio.run(self.stop_async(force, timeout))
    
    async def restart_async(self, timeout: float = 30.0) -> bool:
        """
        异步重启Hub服务器
        
        Args:
            timeout: 重启超时时间
            
        Returns:
            bool: 是否重启成功
        """
        logger.info(f"Restarting Hub '{self.package_name}'")
        
        # 停止当前进程
        if not await self.stop_async():
            logger.error(f"Failed to stop Hub '{self.package_name}' for restart")
            return False
        
        # TODO: 重新启动逻辑
        # 这需要重新生成脚本和配置文件，然后启动新进程
        # 当前版本中，重启需要通过HubPackage.start_server重新实现
        logger.warning(f"Hub restart for '{self.package_name}' requires manual restart via HubPackage")
        return False
    
    def restart(self, timeout: float = 30.0) -> bool:
        """
        同步重启Hub服务器
        
        Args:
            timeout: 重启超时时间
            
        Returns:
            bool: 是否重启成功
        """
        return asyncio.run(self.restart_async(timeout))
    
    async def _wait_for_process_exit(self):
        """等待进程退出"""
        while self.process.poll() is None:
            await asyncio.sleep(0.1)
    
    async def _cleanup_resources(self):
        """清理资源文件"""
        try:
            # 清理临时配置文件
            if os.path.exists(self.config_file):
                os.unlink(self.config_file)
                logger.debug(f"Cleaned up config file: {self.config_file}")
            
            # 清理临时脚本文件
            if os.path.exists(self.script_file):
                os.unlink(self.script_file)
                logger.debug(f"Cleaned up script file: {self.script_file}")
                
        except Exception as e:
            logger.warning(f"Failed to cleanup resources for '{self.package_name}': {e}")
    
    def get_info(self) -> HubProcessInfo:
        """
        获取进程详细信息
        
        Returns:
            HubProcessInfo: 进程信息对象
        """
        return HubProcessInfo(
            package_name=self.package_name,
            port=self.port,
            pid=self.process.pid if self.process else None,
            is_running=self.is_running,
            start_time=self.start_time,
            uptime=self.uptime,
            endpoint_url=self.endpoint_url,
            config_file=self.config_file,
            script_file=self.script_file
        )
    
    def get_status_dict(self) -> Dict[str, Any]:
        """
        获取状态字典
        
        Returns:
            Dict[str, Any]: 包含所有状态信息的字典
        """
        return {
            "package_name": self.package_name,
            "status": self.status.value,
            "port": self.port,
            "endpoint_url": self.endpoint_url,
            "is_running": self.is_running,
            "pid": self.process.pid if self.process else None,
            "uptime": self.uptime,
            "start_time": self.start_time.isoformat() if self.start_time else None,
            "services_count": len(self.services),
            "service_names": [s.name for s in self.services]
        }
    
    def get_available_routes(self) -> List[HubRouteInfo]:
        """
        获取可用的路由信息
        
        基于方案1的基础路由功能，提供：
        - 基础路由：访问所有工具
        - 服务级路由：访问特定服务的工具（当有多个服务时）
        
        Returns:
            List[HubRouteInfo]: 路由信息列表
        """
        routes = []
        base_url = f"http://localhost:{self.port}"
        
        # 基础路由：访问所有工具
        routes.append(HubRouteInfo(
            route_type="basic",
            path=f"{base_url}/mcp",
            description=f"Access all tools from {len(self.services)} services"
        ))
        
        # 如果有多个服务，添加服务级路由（这是一个概念性设计，实际实现取决于MCP服务器的路由能力）
        if len(self.services) > 1:
            for service in self.services:
                routes.append(HubRouteInfo(
                    route_type="service",
                    path=f"{base_url}/mcp/{service.name}",
                    description=f"Access tools from service '{service.name}' ({len(service.tools)} tools)",
                    service_name=service.name
                ))
        
        return routes
    
    def __del__(self):
        """析构函数，确保资源清理"""
        if hasattr(self, 'process') and self.process and self.is_running:
            try:
                self.process.terminate()
                logger.debug(f"Terminated Hub process '{self.package_name}' in destructor")
            except Exception:
                pass
