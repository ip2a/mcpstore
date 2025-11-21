"""
Hub Package Module
Hub包模块 - 管理可启动的Hub服务包
"""

import logging
import socket
from typing import List, Optional

from .process import HubProcess
from .server import HubServerGenerator
from .types import HubConfig, HubServiceInfo, HubStartMode

logger = logging.getLogger(__name__)


class HubPackage:
    """
    Hub服务包
    
    封装了一组服务的Hub配置，可以启动为独立的MCP服务进程。
    每个Hub包代表一个可部署的服务集合。
    
    特点：
    - 包含完整的服务配置信息
    - 支持多种启动模式（当前仅支持subprocess）
    - 自动端口分配和管理
    - 基于FastMCP的服务器生成
    """
    
    def __init__(self, package_name: str, services: List[HubServiceInfo], config: HubConfig):
        """
        初始化Hub服务包
        
        Args:
            package_name: 包名
            services: 服务列表
            config: Hub配置
        """
        self.package_name = package_name
        self.services = services
        self.config = config
        self._process: Optional[HubProcess] = None
        self._server_generator = HubServerGenerator()
        
        logger.info(f"HubPackage '{package_name}' created with {len(services)} services")
    
    async def start_server_async(
        self, 
        port: Optional[int] = None, 
        mode: HubStartMode = HubStartMode.SUBPROCESS
    ) -> HubProcess:
        """
        异步启动Hub服务器
        
        生成FastMCP服务器脚本并启动独立进程提供MCP服务。
        
        Args:
            port: 端口号，None则自动分配
            mode: 启动模式，目前仅支持SUBPROCESS
            
        Returns:
            HubProcess: Hub进程管理器
            
        Raises:
            ValueError: 当模式不支持时
            RuntimeError: 当启动失败时
        """
        try:
            logger.info(f"Starting Hub server '{self.package_name}' in {mode.value} mode")
            
            # 1. 端口分配
            if port is None:
                port = self._allocate_port()
            logger.debug(f"Using port: {port}")
            
            # 2. 检查启动模式
            if mode != HubStartMode.SUBPROCESS:
                raise ValueError(f"Start mode {mode.value} not implemented yet")
            
            # 3. 启动子进程
            hub_process = await self._start_subprocess(port)
            
            # 4. 缓存进程引用
            self._process = hub_process
            
            logger.info(f"Hub server '{self.package_name}' started successfully on port {port}")
            return hub_process
            
        except Exception as e:
            logger.error(f"Failed to start Hub server '{self.package_name}': {e}")
            raise RuntimeError(f"Hub server startup failed: {e}") from e
    
    def start_server(
        self, 
        port: Optional[int] = None, 
        mode: HubStartMode = HubStartMode.SUBPROCESS
    ) -> HubProcess:
        """
        同步启动Hub服务器
        
        这是start_server_async的同步版本。
        
        Args:
            port: 端口号，None则自动分配
            mode: 启动模式
            
        Returns:
            HubProcess: Hub进程管理器
        """
        # 使用asyncio运行异步方法
        import asyncio
        
        # 检查是否在异步上下文中
        try:
            loop = asyncio.get_running_loop()
            # 如果在异步上下文中，使用线程池执行
            import concurrent.futures
            with concurrent.futures.ThreadPoolExecutor() as executor:
                future = executor.submit(
                    lambda: asyncio.run(self.start_server_async(port, mode))
                )
                return future.result()
        except RuntimeError:
            # 如果不在异步上下文中，直接运行
            return asyncio.run(self.start_server_async(port, mode))
    
    def _allocate_port(self) -> int:
        """
        自动分配可用端口
        
        在3000-4000范围内查找可用端口。
        
        Returns:
            int: 可用的端口号
            
        Raises:
            RuntimeError: 当无可用端口时
        """
        # 如果配置中指定了端口，优先使用
        if self.config.port:
            if self._is_port_available(self.config.port):
                return self.config.port
            else:
                logger.warning(f"Configured port {self.config.port} is not available, auto-allocating")
        
        # 自动分配端口
        for port in range(3000, 4000):
            if self._is_port_available(port):
                logger.debug(f"Allocated port: {port}")
                return port
        
        raise RuntimeError("No available ports in range 3000-4000")
    
    def _is_port_available(self, port: int) -> bool:
        """
        检查端口是否可用
        
        Args:
            port: 端口号
            
        Returns:
            bool: 端口是否可用
        """
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(('localhost', port))
                return True
        except OSError:
            return False
    
    async def _start_subprocess(self, port: int) -> HubProcess:
        """
        通过子进程启动Hub服务器
        
        Args:
            port: 端口号
            
        Returns:
            HubProcess: Hub进程管理器
        """
        try:
            # 1. 生成服务器脚本和配置
            script_file, config_file = await self._server_generator.generate_server_files_async(
                package_name=self.package_name,
                services=self.services,
                config=self.config,
                port=port
            )
            
            # 2. 启动子进程
            process = await self._server_generator.start_subprocess_async(
                script_file=script_file,
                config_file=config_file,
                port=port
            )
            
            # 3. 创建进程管理器
            hub_process = HubProcess(
                package_name=self.package_name,
                process=process,
                port=port,
                config_file=config_file,
                script_file=script_file,
                services=self.services
            )
            
            # 4. 等待服务器启动
            await hub_process.wait_for_startup()
            
            return hub_process
            
        except Exception as e:
            logger.error(f"Failed to start subprocess for Hub '{self.package_name}': {e}")
            raise
    
    @property
    def is_running(self) -> bool:
        """
        检查Hub是否正在运行
        
        Returns:
            bool: 是否正在运行
        """
        return self._process is not None and self._process.is_running
    
    @property
    def process(self) -> Optional[HubProcess]:
        """
        获取当前的Hub进程
        
        Returns:
            Optional[HubProcess]: Hub进程管理器，如果未启动则为None
        """
        return self._process
    
    def get_summary(self) -> dict:
        """
        获取Hub包摘要信息
        
        Returns:
            dict: 包含包名、服务数量、配置等信息的摘要
        """
        return {
            "package_name": self.package_name,
            "services_count": len(self.services),
            "service_names": [s.name for s in self.services],
            "config": {
                "name": self.config.name,
                "description": self.config.description,
                "context_type": self.config.context_type,
                "target_id": self.config.target_id,
                "basic_routing": self.config.basic_routing
            },
            "is_running": self.is_running,
            "process_info": self._process.get_info() if self._process else None
        }
