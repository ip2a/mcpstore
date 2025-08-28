"""
Local Service Adapter
Provides backward compatibility while transitioning from LocalServiceManager to FastMCP.
"""

import logging
from typing import Dict, Any, Optional, Tuple
from pathlib import Path
from .fastmcp_integration import FastMCPServiceManager, get_fastmcp_service_manager

logger = logging.getLogger(__name__)

class LocalServiceManagerAdapter:
    """
    LocalServiceManager适配器
    
    提供与原LocalServiceManager相同的接口，但内部使用FastMCP实现。
    这确保了向后兼容性，同时逐步迁移到FastMCP。
    """
    
    def __init__(self, base_work_dir: str = None):
        """
        初始化适配器
        
        Args:
            base_work_dir: 基础工作目录
        """
        self.base_work_dir = Path(base_work_dir or Path.cwd())
        
        # 使用FastMCP服务管理器作为底层实现
        self.fastmcp_manager = FastMCPServiceManager(self.base_work_dir)
        
        # 健康检查配置
        self.health_check_interval = 30
        self.max_restart_attempts = 3
        self.restart_delay = 5

        # 监控任务
        self._health_check_task = None
        self._monitor_started = False
        
        logger.info(f"LocalServiceManagerAdapter initialized (using FastMCP backend)")
    
    async def start_local_service(self, name: str, config: Dict[str, Any]) -> Tuple[bool, str]:
        """
        启动本地服务（兼容LocalServiceManager接口）
        
        Args:
            name: 服务名称
            config: 服务配置
            
        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        logger.info(f"[Adapter] Starting local service {name} via FastMCP")
        
        # 委托给FastMCP管理器
        return await self.fastmcp_manager.start_local_service(name, config)
    
    async def stop_local_service(self, name: str) -> Tuple[bool, str]:
        """
        停止本地服务（兼容LocalServiceManager接口）
        
        Args:
            name: 服务名称
            
        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        logger.info(f"[Adapter] Stopping local service {name} via FastMCP")
        
        # 委托给FastMCP管理器
        return await self.fastmcp_manager.stop_local_service(name)
    
    def get_service_status(self, name: str) -> Dict[str, Any]:
        """
        获取服务状态（兼容LocalServiceManager接口）
        
        Args:
            name: 服务名称
            
        Returns:
            Dict[str, Any]: 服务状态信息
        """
        # 委托给FastMCP管理器
        status = self.fastmcp_manager.get_service_status(name)
        
        # 转换为原LocalServiceManager的状态格式
        if status.get("status") == "not_found":
            return {"status": "not_found"}
        elif status.get("status") == "error":
            return {"status": "stopped", "error": status.get("error")}
        else:
            return {
                "status": "running",
                "pid": 0,  # FastMCP管理的进程，不暴露PID
                "start_time": status.get("start_time", 0),
                "restart_count": 0,  # FastMCP自动处理重启
                "uptime": status.get("uptime", 0),
                "managed_by": "fastmcp"
            }
    
    def list_services(self) -> Dict[str, Dict[str, Any]]:
        """
        列出所有服务状态（兼容LocalServiceManager接口）
        
        Returns:
            Dict[str, Dict[str, Any]]: 所有服务的状态信息
        """
        # 委托给FastMCP管理器，并转换格式
        fastmcp_services = self.fastmcp_manager.list_services()
        
        # 转换为原LocalServiceManager的格式
        result = {}
        for name, status in fastmcp_services.items():
            result[name] = self.get_service_status(name)
        
        return result
    
    async def cleanup(self):
        """
        清理所有服务（兼容LocalServiceManager接口）
        """
        logger.info("[Adapter] Cleaning up services via FastMCP")
        
        # 停止健康监控（兼容性）
        if self._health_check_task:
            self._health_check_task.cancel()
        
        # 委托给FastMCP管理器
        await self.fastmcp_manager.cleanup()
    
    # 健康监控、进程检查、服务重启等功能现在完全由FastMCP自动处理

    async def start_health_monitoring(self):
        """启动健康监控（FastMCP自动处理）"""
        logger.info("[Adapter] Health monitoring delegated to FastMCP")
        self._monitor_started = True
    
    # _prepare_environment和_resolve_working_dir方法已删除
    # 环境变量和工作目录处理现在完全由FastMCP配置规范化处理

# 全局实例（保持与原LocalServiceManager相同的接口）
_local_service_manager_adapter: Optional[LocalServiceManagerAdapter] = None


def get_local_service_manager() -> LocalServiceManagerAdapter:
    """
    获取全局本地服务管理器实例（适配器版本）
    
    这个函数替代了原来的get_local_service_manager，但返回适配器实例。
    适配器提供相同的接口，但内部使用FastMCP实现。
    
    Returns:
        LocalServiceManagerAdapter: 全局适配器实例
    """
    global _local_service_manager_adapter
    if _local_service_manager_adapter is None:
        _local_service_manager_adapter = LocalServiceManagerAdapter()
    return _local_service_manager_adapter


def set_local_service_manager_work_dir(base_work_dir: str):
    """
    设置本地服务管理器的工作目录（用于数据空间模式）
    
    Args:
        base_work_dir: 基础工作目录
    """
    global _local_service_manager_adapter
    _local_service_manager_adapter = LocalServiceManagerAdapter(base_work_dir)
    logger.info(f"LocalServiceManagerAdapter work directory set to: {base_work_dir}")

# 导出适配器类
LocalServiceManager = LocalServiceManagerAdapter

# LocalServiceProcess类（用于类型兼容）
from dataclasses import dataclass
import subprocess

@dataclass
class LocalServiceProcess:
    """Local service process information"""
    name: str
    process: Optional[subprocess.Popen] = None
    config: Dict[str, Any] = None
    start_time: float = 0
    pid: int = 0
    status: str = "running"
    restart_count: int = 0
    last_health_check: float = 0

