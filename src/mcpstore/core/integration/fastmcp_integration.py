"""
FastMCP Integration Layer
Provides a clean interface between MCPStore and FastMCP, handling configuration normalization.
"""

import asyncio
import logging
from typing import Dict, Any, List, Optional, Tuple
from pathlib import Path
from fastmcp import Client
import time

logger = logging.getLogger(__name__)

class FastMCPServiceManager:
    """
    FastMCP服务管理器
    
    负责将MCPStore的宽松配置转换为FastMCP标准配置，并管理FastMCP客户端。
    这是MCPStore和FastMCP之间的桥梁。
    """
    
    def __init__(self, base_work_dir: Optional[Path] = None):
        """
        初始化FastMCP服务管理器
        
        Args:
            base_work_dir: 基础工作目录，用于本地服务
        """
        self.base_work_dir = base_work_dir or Path.cwd()
        self.clients: Dict[str, Client] = {}
        self.service_configs: Dict[str, Dict[str, Any]] = {}
        self.service_start_times: Dict[str, float] = {}
        
        logger.info(f"FastMCPServiceManager initialized with work_dir: {self.base_work_dir}")
    
    async def start_local_service(self, name: str, config: Dict[str, Any]) -> Tuple[bool, str]:
        """
        启动本地服务（替代LocalServiceManager.start_local_service）
        
        Args:
            name: 服务名称
            config: 用户配置（宽松格式）
            
        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        try:
            logger.info(f"Starting local service {name} with FastMCP")
            
            # 1. 配置规范化：将用户配置转换为FastMCP标准格式
            fastmcp_config = self._normalize_local_service_config(name, config)
            
            # 2. 创建FastMCP客户端
            client = Client(fastmcp_config)
            
            # 3. 测试连接（FastMCP会自动启动进程）
            try:
                async with client:
                    # FastMCP自动处理：
                    # - 进程启动 (subprocess.Popen)
                    # - 环境变量设置
                    # - 工作目录设置
                    # - stdin/stdout管理
                    await client.ping()  # 标准MCP ping
                    
                    # 存储客户端和配置
                    self.clients[name] = client
                    self.service_configs[name] = config
                    self.service_start_times[name] = time.time()
                    
                    logger.info(f"Local service {name} started successfully via FastMCP")
                    return True, f"Service started successfully via FastMCP"
                    
            except Exception as e:
                logger.error(f"FastMCP failed to start service {name}: {e}")
                return False, f"FastMCP connection failed: {str(e)}"
                
        except Exception as e:
            logger.error(f"Failed to start local service {name}: {e}")
            return False, str(e)
    
    async def stop_local_service(self, name: str) -> Tuple[bool, str]:
        """
        停止本地服务（替代LocalServiceManager.stop_local_service）
        
        Args:
            name: 服务名称
            
        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        try:
            if name not in self.clients:
                return False, f"Service {name} not found"
            
            # FastMCP客户端会自动处理进程清理
            client = self.clients[name]
            
            # 清理记录
            del self.clients[name]
            if name in self.service_configs:
                del self.service_configs[name]
            if name in self.service_start_times:
                del self.service_start_times[name]
            
            logger.info(f"Local service {name} stopped successfully")
            return True, "Service stopped successfully"
            
        except Exception as e:
            logger.error(f"Failed to stop local service {name}: {e}")
            return False, str(e)
    
    def get_service_status(self, name: str) -> Dict[str, Any]:
        """
        获取服务状态（替代LocalServiceManager.get_service_status）
        
        Args:
            name: 服务名称
            
        Returns:
            Dict[str, Any]: 服务状态信息
        """
        if name not in self.clients:
            return {"status": "not_found"}
        
        try:
            # 使用FastMCP客户端检查连接状态
            client = self.clients[name]
            
            # 简单的状态检查
            start_time = self.service_start_times.get(name, 0)
            uptime = time.time() - start_time if start_time > 0 else 0
            
            return {
                "status": "running",  # FastMCP管理的服务假设为运行状态
                "uptime": uptime,
                "start_time": start_time,
                "managed_by": "fastmcp"
            }
            
        except Exception as e:
            logger.error(f"Failed to get service status for {name}: {e}")
            return {"status": "error", "error": str(e)}
    
    def list_services(self) -> Dict[str, Dict[str, Any]]:
        """
        列出所有服务状态（替代LocalServiceManager.list_services）
        
        Returns:
            Dict[str, Dict[str, Any]]: 所有服务的状态信息
        """
        return {name: self.get_service_status(name) for name in self.clients}
    
    async def cleanup(self):
        """
        清理所有服务（替代LocalServiceManager.cleanup）
        """
        logger.info("Cleaning up FastMCP services...")
        
        # 停止所有服务
        for name in list(self.clients.keys()):
            await self.stop_local_service(name)
        
        logger.info("FastMCP service cleanup completed")
    
    def _normalize_local_service_config(self, name: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        配置规范化：将MCPStore的宽松配置转换为FastMCP标准配置
        
        这是MCPStore的核心价值：允许用户输入宽松格式，转换为标准格式
        
        Args:
            name: 服务名称
            config: 用户配置（宽松格式）
            
        Returns:
            Dict[str, Any]: FastMCP标准配置
        """
        # FastMCP标准配置格式
        fastmcp_config = {
            "mcpServers": {
                name: {}
            }
        }
        
        service_config = fastmcp_config["mcpServers"][name]
        
        # 1. 处理必需字段
        if "command" not in config:
            raise ValueError(f"Local service {name} missing required 'command' field")
        
        service_config["command"] = config["command"]
        
        # 2. 处理可选字段
        if "args" in config:
            service_config["args"] = config["args"]
        
        # 3. 环境变量处理（简化版）
        env = {}
        if "env" in config:
            env.update(config["env"])
        
        # 确保PYTHONPATH包含工作目录
        if "PYTHONPATH" not in env:
            env["PYTHONPATH"] = str(self.base_work_dir)
        else:
            env["PYTHONPATH"] = f"{self.base_work_dir}{Path.pathsep}{env['PYTHONPATH']}"
        
        service_config["env"] = env
        
        # 4. 工作目录处理
        working_dir = config.get("working_dir")
        if working_dir:
            # 如果是相对路径，相对于base_work_dir
            work_path = Path(working_dir)
            if not work_path.is_absolute():
                work_path = self.base_work_dir / work_path
            service_config["cwd"] = str(work_path.resolve())
        else:
            service_config["cwd"] = str(self.base_work_dir)
        
        logger.debug(f"Normalized config for {name}: {fastmcp_config}")
        return fastmcp_config

# 全局实例（保持与LocalServiceManager相同的接口）
_fastmcp_service_manager: Optional[FastMCPServiceManager] = None

def get_fastmcp_service_manager(base_work_dir: Optional[Path] = None) -> FastMCPServiceManager:
    """
    获取全局FastMCP服务管理器实例（替代get_local_service_manager）
    
    Args:
        base_work_dir: 基础工作目录
        
    Returns:
        FastMCPServiceManager: 全局实例
    """
    global _fastmcp_service_manager
    if _fastmcp_service_manager is None:
        _fastmcp_service_manager = FastMCPServiceManager(base_work_dir)
    elif base_work_dir and _fastmcp_service_manager.base_work_dir != base_work_dir:
        # 如果工作目录不同，创建新实例
        _fastmcp_service_manager = FastMCPServiceManager(base_work_dir)
    return _fastmcp_service_manager

