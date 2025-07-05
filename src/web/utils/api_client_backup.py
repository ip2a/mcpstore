"""
MCPStore API客户端
封装所有API调用逻辑，支持HTTP API和直接方法调用两种模式
"""

import requests
import json
from typing import Dict, List, Optional, Any
import streamlit as st
from abc import ABC, abstractmethod
from datetime import datetime

class MCPStoreBackend(ABC):
    """MCPStore后端抽象基类"""

    @abstractmethod
    def test_connection(self) -> bool:
        """测试连接"""
        pass

    @abstractmethod
    def list_services(self) -> Optional[Dict]:
        """获取服务列表"""
        pass

    @abstractmethod
    def add_service(self, service_config: Dict) -> Optional[Dict]:
        """添加服务"""
        pass

class HTTPBackend(MCPStoreBackend):
    """HTTP API后端实现"""

    def __init__(self, base_url: str = "http://localhost:18611"):
        self.base_url = base_url.rstrip('/')
        self.session = requests.Session()
        self.session.headers.update({
            'Content-Type': 'application/json'
        })
        self._connection_status = None
        self._last_check = None

    def _request(self, method: str, endpoint: str, **kwargs) -> Optional[Dict]:
        """发送HTTP请求"""
        url = f"{self.base_url}{endpoint}"

        try:
            response = self.session.request(method, url, timeout=10, **kwargs)
            response.raise_for_status()

            # 更新连接状态
            self._connection_status = True
            self._last_check = datetime.now()

            return response.json()
        except requests.exceptions.RequestException as e:
            self._connection_status = False
            self._last_check = datetime.now()
            st.error(f"API请求失败: {e}")
            return None
        except json.JSONDecodeError:
            st.error("API响应格式错误")
            return None

    def test_connection(self) -> bool:
        """测试API连接"""
        try:
            response = self._request('GET', '/for_store/health')
            return response is not None
        except:
            return False

    def get_connection_status(self) -> Dict:
        """获取连接状态信息"""
        return {
            'status': self._connection_status,
            'last_check': self._last_check,
            'base_url': self.base_url
        }
    
    # ==================== Store级别API ====================

    def list_services(self) -> Optional[Dict]:
        """获取服务列表"""
        return self._request('GET', '/for_store/list_services')

    def add_service(self, service_config: Dict) -> Optional[Dict]:
        """添加服务"""
        return self._request('POST', '/for_store/add_service', json=service_config)
    
    def delete_service(self, service_name: str) -> Optional[Dict]:
        """删除服务"""
        return self._request('POST', '/for_store/delete_service', json={"name": service_name})
    
    def update_service(self, service_name: str, config: Dict) -> Optional[Dict]:
        """更新服务"""
        data = {"name": service_name, **config}
        return self._request('POST', '/for_store/update_service', json=data)
    
    def restart_service(self, service_name: str) -> Optional[Dict]:
        """重启服务"""
        return self._request('POST', '/for_store/restart_service', json={"name": service_name})
    
    def get_service_info(self, service_name: str) -> Optional[Dict]:
        """获取服务信息"""
        return self._request('POST', '/for_store/get_service_info', json={"name": service_name})
    
    def get_service_status(self, service_name: str) -> Optional[Dict]:
        """获取服务状态"""
        return self._request('POST', '/for_store/get_service_status', json={"name": service_name})
    
    def check_services(self) -> Optional[Dict]:
        """检查所有服务"""
        return self._request('GET', '/for_store/check_services')
    
    def batch_add_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量添加服务"""
        return self._request('POST', '/for_store/batch_add_services', json={"services": services})

    def batch_update_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量更新服务"""
        return self._request('POST', '/for_store/batch_update_services', json={"services": services})

    def batch_restart_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量重启服务"""
        return self._request('POST', '/for_store/batch_restart_services', json={"service_names": service_names})

    def batch_delete_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量删除服务"""
        return self._request('POST', '/for_store/batch_delete_services', json={"service_names": service_names})
    
    # ==================== 工具管理API ====================
    
    def list_tools(self) -> Optional[Dict]:
        """获取工具列表"""
        return self._request('GET', '/for_store/list_tools')
    
    def use_tool(self, tool_name: str, args: Dict) -> Optional[Dict]:
        """使用工具"""
        data = {"tool_name": tool_name, "args": args}
        return self._request('POST', '/for_store/use_tool', json=data)
    
    # ==================== 配置管理API ====================
    
    def get_config(self) -> Optional[Dict]:
        """获取配置"""
        return self._request('GET', '/for_store/get_config')
    
    def show_mcpconfig(self) -> Optional[Dict]:
        """显示MCP配置"""
        return self._request('GET', '/for_store/show_mcpconfig')
    
    def reset_config(self) -> Optional[Dict]:
        """重置配置"""
        return self._request('POST', '/for_store/reset_config')
    
    # ==================== 监控API ====================

    def get_stats(self) -> Optional[Dict]:
        """获取统计信息"""
        return self._request('GET', '/for_store/get_stats')

    def get_monitoring_status(self) -> Optional[Dict]:
        """获取监控状态"""
        return self._request('GET', '/monitoring/status')

    def update_monitoring_config(self, config: Dict) -> Optional[Dict]:
        """更新监控配置"""
        return self._request('POST', '/monitoring/config', json=config)

    def restart_monitoring(self) -> Optional[Dict]:
        """重启监控任务"""
        return self._request('POST', '/monitoring/restart')

    def get_health(self) -> Optional[Dict]:
        """获取系统健康状态"""
        return self._request('GET', '/for_store/health')

    # ==================== 配置验证和状态查询API ====================

    def validate_config(self) -> Optional[Dict]:
        """验证Store配置"""
        return self._request('GET', '/for_store/validate_config')

    def get_service_status(self, service_name: str) -> Optional[Dict]:
        """获取服务详细状态"""
        return self._request('POST', '/for_store/get_service_status', json={"name": service_name})

    def validate_agent_config(self, agent_id: str) -> Optional[Dict]:
        """验证Agent配置"""
        return self._request('GET', f'/for_agent/{agent_id}/validate_config')

    def get_agent_config(self, agent_id: str) -> Optional[Dict]:
        """获取Agent配置"""
        return self._request('GET', f'/for_agent/{agent_id}/get_config')

    def show_agent_mcpconfig(self, agent_id: str) -> Optional[Dict]:
        """显示Agent MCP配置"""
        return self._request('GET', f'/for_agent/{agent_id}/show_mcpconfig')

    def update_agent_config(self, agent_id: str, config: Dict) -> Optional[Dict]:
        """更新Agent配置"""
        return self._request('POST', f'/for_agent/{agent_id}/update_config', json={"config": config})

    # ==================== 服务管理API ====================

    def delete_service(self, service_name: str) -> Optional[Dict]:
        """删除服务"""
        data = {"name": service_name}
        return self._request('POST', '/for_store/delete_service', json=data)

    def update_service(self, service_name: str, config: Dict) -> Optional[Dict]:
        """更新服务配置"""
        data = {"name": service_name, "config": config}
        return self._request('POST', '/for_store/update_service', json=data)

    def restart_service(self, service_name: str) -> Optional[Dict]:
        """重启服务"""
        data = {"name": service_name}
        return self._request('POST', '/for_store/restart_service', json=data)

    def batch_add_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量添加服务"""
        data = {"services": services}
        return self._request('POST', '/for_store/batch_add_services', json=data)

    # ==================== Agent管理API ====================

    def list_agent_services(self, agent_id: str) -> Optional[Dict]:
        """获取Agent服务列表"""
        return self._request('GET', f'/for_agent/{agent_id}/list_services')

    def add_agent_service(self, agent_id: str, service_names: List[str]) -> Optional[Dict]:
        """为Agent添加服务"""
        return self._request('POST', f'/for_agent/{agent_id}/add_service', json=service_names)

    def list_agent_tools(self, agent_id: str) -> Optional[Dict]:
        """获取Agent工具列表"""
        return self._request('GET', f'/for_agent/{agent_id}/list_tools')

    def delete_agent_service(self, agent_id: str, service_name: str) -> Optional[Dict]:
        """删除Agent服务"""
        data = {"name": service_name}
        return self._request('POST', f'/for_agent/{agent_id}/delete_service', json=data)

    def reset_agent_config(self, agent_id: str) -> Optional[Dict]:
        """重置Agent配置"""
        return self._request('POST', f'/for_agent/{agent_id}/reset_config')

    def get_agent_stats(self, agent_id: str) -> Optional[Dict]:
        """获取Agent统计信息"""
        return self._request('GET', f'/for_agent/{agent_id}/get_stats')
    
    def get_health(self) -> Optional[Dict]:
        """获取健康状态"""
        return self._request('GET', '/for_store/health')
    
    def get_monitoring_status(self) -> Optional[Dict]:
        """获取监控状态"""
        return self._request('GET', '/monitoring/status')
    
    def update_monitoring_config(self, config: Dict) -> Optional[Dict]:
        """更新监控配置"""
        return self._request('POST', '/monitoring/config', json=config)
    
    def restart_monitoring(self) -> Optional[Dict]:
        """重启监控"""
        return self._request('POST', '/monitoring/restart')
    
    # ==================== Agent级别API ====================
    
    def list_agent_services(self, agent_id: str) -> Optional[Dict]:
        """获取Agent服务列表"""
        return self._request('GET', f'/for_agent/{agent_id}/list_services')
    
    def add_agent_service(self, agent_id: str, service_config) -> Optional[Dict]:
        """为Agent添加服务"""
        return self._request('POST', f'/for_agent/{agent_id}/add_service', json=service_config)
    
    def delete_agent_service(self, agent_id: str, service_name: str) -> Optional[Dict]:
        """删除Agent服务"""
        return self._request('POST', f'/for_agent/{agent_id}/delete_service', json={"name": service_name})
    
    def list_agent_tools(self, agent_id: str) -> Optional[Dict]:
        """获取Agent工具列表"""
        return self._request('GET', f'/for_agent/{agent_id}/list_tools')
    
    def use_agent_tool(self, agent_id: str, tool_name: str, args: Dict) -> Optional[Dict]:
        """使用Agent工具"""
        data = {"tool_name": tool_name, "args": args}
        return self._request('POST', f'/for_agent/{agent_id}/use_tool', json=data)
    
    def get_agent_config(self, agent_id: str) -> Optional[Dict]:
        """获取Agent配置"""
        return self._request('GET', f'/for_agent/{agent_id}/get_config')
    
    def reset_agent_config(self, agent_id: str) -> Optional[Dict]:
        """重置Agent配置"""
        return self._request('POST', f'/for_agent/{agent_id}/reset_config')
    
    def get_agent_stats(self, agent_id: str) -> Optional[Dict]:
        """获取Agent统计"""
        return self._request('GET', f'/for_agent/{agent_id}/get_stats')
    
    def get_agent_health(self, agent_id: str) -> Optional[Dict]:
        """获取Agent健康状态"""
        return self._request('GET', f'/for_agent/{agent_id}/health')
    
    # ==================== 通用API ====================
    
    def get_service_by_name(self, service_name: str, agent_id: Optional[str] = None) -> Optional[Dict]:
        """通过名称获取服务"""
        url = f'/services/{service_name}'
        if agent_id:
            url += f'?agent_id={agent_id}'
        return self._request('GET', url)

class DirectBackend(MCPStoreBackend):
    """直接方法调用后端实现（用于后期无缝衔接）"""

    def __init__(self):
        self._mcpstore = None
        self._connection_status = False

    def _init_mcpstore(self):
        """初始化MCPStore实例"""
        try:
            # 这里将来会导入实际的MCPStore
            # from mcpstore import MCPStore
            # self._mcpstore = MCPStore.setup_store()
            # self._connection_status = True

            # 目前返回模拟状态
            self._connection_status = False
            return False
        except ImportError:
            self._connection_status = False
            return False

    def test_connection(self) -> bool:
        """测试连接"""
        if self._mcpstore is None:
            return self._init_mcpstore()
        return self._connection_status

    def list_services(self) -> Optional[Dict]:
        """获取服务列表"""
        if not self.test_connection():
            return None

        try:
            # 将来的实现：
            # services = await self._mcpstore.for_store().list_services()
            # return {"success": True, "data": services}

            # 目前返回空结果
            return {"success": True, "data": []}
        except Exception as e:
            st.error(f"获取服务列表失败: {e}")
            return None

    def add_service(self, service_config: Dict) -> Optional[Dict]:
        """添加服务"""
        if not self.test_connection():
            return None

        try:
            # 将来的实现：
            # result = await self._mcpstore.for_store().add_service(service_config)
            # return {"success": True, "data": result}

            # 目前返回模拟结果
            return {"success": True, "data": True}
        except Exception as e:
            st.error(f"添加服务失败: {e}")
            return None

class MCPStoreAPI:
    """MCPStore API统一接口"""

    def __init__(self, backend_type: str = "http", base_url: str = "http://localhost:18611"):
        """
        初始化API客户端

        Args:
            backend_type: 后端类型 ("http" 或 "direct")
            base_url: HTTP后端的基础URL
        """
        if backend_type == "http":
            self.backend = HTTPBackend(base_url)
        elif backend_type == "direct":
            self.backend = DirectBackend()
        else:
            raise ValueError(f"不支持的后端类型: {backend_type}")

        self.backend_type = backend_type

    def switch_backend(self, backend_type: str, base_url: str = None):
        """切换后端类型"""
        if backend_type == "http":
            self.backend = HTTPBackend(base_url or "http://localhost:18611")
        elif backend_type == "direct":
            self.backend = DirectBackend()
        else:
            raise ValueError(f"不支持的后端类型: {backend_type}")

        self.backend_type = backend_type

    def get_backend_info(self) -> Dict:
        """获取后端信息"""
        info = {
            "type": self.backend_type,
            "status": "unknown"
        }

        if hasattr(self.backend, 'get_connection_status'):
            info.update(self.backend.get_connection_status())

        return info

    # ==================== 委托所有API方法给后端 ====================

    def test_connection(self) -> bool:
        """测试连接"""
        return self.backend.test_connection()

    def list_services(self) -> Optional[Dict]:
        """获取服务列表"""
        return self.backend.list_services()

    def add_service(self, service_config: Dict) -> Optional[Dict]:
        """添加服务"""
        return self.backend.add_service(service_config)

    # 对于HTTP后端，委托给HTTPBackend的方法
    def _delegate_to_http(self, method_name: str, *args, **kwargs):
        """委托方法给HTTP后端"""
        if isinstance(self.backend, HTTPBackend):
            method = getattr(self.backend, method_name, None)
            if method:
                return method(*args, **kwargs)
        return None

    # ==================== Store级别API ====================

    def delete_service(self, service_name: str) -> Optional[Dict]:
        """删除服务"""
        return self._delegate_to_http('delete_service', service_name)

    def update_service(self, service_name: str, config: Dict) -> Optional[Dict]:
        """更新服务"""
        return self._delegate_to_http('update_service', service_name, config)

    def restart_service(self, service_name: str) -> Optional[Dict]:
        """重启服务"""
        return self._delegate_to_http('restart_service', service_name)

    def get_service_info(self, service_name: str) -> Optional[Dict]:
        """获取服务信息"""
        return self._delegate_to_http('get_service_info', service_name)

    def get_service_status(self, service_name: str) -> Optional[Dict]:
        """获取服务状态"""
        return self._delegate_to_http('get_service_status', service_name)

    def check_services(self) -> Optional[Dict]:
        """检查所有服务"""
        return self._delegate_to_http('check_services')

    def batch_add_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量添加服务"""
        return self._delegate_to_http('batch_add_services', services)

    def batch_update_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量更新服务"""
        return self._delegate_to_http('batch_update_services', services)

    def batch_restart_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量重启服务"""
        return self._delegate_to_http('batch_restart_services', service_names)

    def batch_delete_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量删除服务"""
        return self._delegate_to_http('batch_delete_services', service_names)

    # ==================== 工具管理API ====================

    def list_tools(self) -> Optional[Dict]:
        """获取工具列表"""
        return self._delegate_to_http('list_tools')

    def use_tool(self, tool_name: str, args: Dict) -> Optional[Dict]:
        """使用工具"""
        return self._delegate_to_http('use_tool', tool_name, args)

    # ==================== 配置管理API ====================

    def get_config(self) -> Optional[Dict]:
        """获取配置"""
        return self._delegate_to_http('get_config')

    def show_mcpconfig(self) -> Optional[Dict]:
        """显示MCP配置"""
        return self._delegate_to_http('show_mcpconfig')

    def reset_config(self) -> Optional[Dict]:
        """重置配置"""
        return self._delegate_to_http('reset_config')

    # ==================== 监控管理API ====================

    def get_monitoring_status(self) -> Optional[Dict]:
        """获取监控状态"""
        return self._delegate_to_http('get_monitoring_status')

    def update_monitoring_config(self, config: Dict) -> Optional[Dict]:
        """更新监控配置"""
        return self._delegate_to_http('update_monitoring_config', config)

    def restart_monitoring(self) -> Optional[Dict]:
        """重启监控任务"""
        return self._delegate_to_http('restart_monitoring')

    def get_health(self) -> Optional[Dict]:
        """获取系统健康状态"""
        return self._delegate_to_http('get_health')

    def health(self) -> Optional[Dict]:
        """获取健康状态（别名方法）"""
        return self.get_health()

    # ==================== 服务管理API ====================

    def delete_service(self, service_name: str) -> Optional[Dict]:
        """删除服务"""
        return self._delegate_to_http('delete_service', service_name)

    def update_service(self, service_name: str, config: Dict) -> Optional[Dict]:
        """更新服务配置"""
        return self._delegate_to_http('update_service', service_name, config)

    def restart_service(self, service_name: str) -> Optional[Dict]:
        """重启服务"""
        return self._delegate_to_http('restart_service', service_name)

    def batch_add_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量添加服务"""
        return self._delegate_to_http('batch_add_services', services)

    # ==================== Agent管理API ====================

    def list_agent_services(self, agent_id: str) -> Optional[Dict]:
        """获取Agent服务列表"""
        return self._delegate_to_http('list_agent_services', agent_id)

    def add_agent_service(self, agent_id: str, service_names: List[str]) -> Optional[Dict]:
        """为Agent添加服务"""
        return self._delegate_to_http('add_agent_service', agent_id, service_names)

    def list_agent_tools(self, agent_id: str) -> Optional[Dict]:
        """获取Agent工具列表"""
        return self._delegate_to_http('list_agent_tools', agent_id)

    def delete_agent_service(self, agent_id: str, service_name: str) -> Optional[Dict]:
        """删除Agent服务"""
        return self._delegate_to_http('delete_agent_service', agent_id, service_name)

    def reset_agent_config(self, agent_id: str) -> Optional[Dict]:
        """重置Agent配置"""
        return self._delegate_to_http('reset_agent_config', agent_id)

    def get_agent_stats(self, agent_id: str) -> Optional[Dict]:
        """获取Agent统计信息"""
        return self._delegate_to_http('get_agent_stats', agent_id)

    # ==================== 配置验证和状态查询API ====================

    def validate_config(self) -> Optional[Dict]:
        """验证Store配置"""
        return self._delegate_to_http('validate_config')

    def get_service_status(self, service_name: str) -> Optional[Dict]:
        """获取服务详细状态"""
        return self._delegate_to_http('get_service_status', service_name)

    def validate_agent_config(self, agent_id: str) -> Optional[Dict]:
        """验证Agent配置"""
        return self._delegate_to_http('validate_agent_config', agent_id)

    def get_agent_config(self, agent_id: str) -> Optional[Dict]:
        """获取Agent配置"""
        return self._delegate_to_http('get_agent_config', agent_id)

    def show_agent_mcpconfig(self, agent_id: str) -> Optional[Dict]:
        """显示Agent MCP配置"""
        return self._delegate_to_http('show_agent_mcpconfig', agent_id)

    def update_agent_config(self, agent_id: str, config: Dict) -> Optional[Dict]:
        """更新Agent配置"""
        return self._delegate_to_http('update_agent_config', agent_id, config)

    # ==================== 监控API ====================

    def get_stats(self) -> Optional[Dict]:
        """获取统计信息"""
        return self._delegate_to_http('get_stats')

    def get_health(self) -> Optional[Dict]:
        """获取健康状态"""
        return self._delegate_to_http('get_health')

    def get_monitoring_status(self) -> Optional[Dict]:
        """获取监控状态"""
        return self._delegate_to_http('get_monitoring_status')

    def update_monitoring_config(self, config: Dict) -> Optional[Dict]:
        """更新监控配置"""
        return self._delegate_to_http('update_monitoring_config', config)

    def restart_monitoring(self) -> Optional[Dict]:
        """重启监控"""
        return self._delegate_to_http('restart_monitoring')

    # ==================== Agent级别API ====================

    def list_agent_services(self, agent_id: str) -> Optional[Dict]:
        """获取Agent服务列表"""
        return self._delegate_to_http('list_agent_services', agent_id)

    def add_agent_service(self, agent_id: str, service_config) -> Optional[Dict]:
        """为Agent添加服务"""
        return self._delegate_to_http('add_agent_service', agent_id, service_config)

    def delete_agent_service(self, agent_id: str, service_name: str) -> Optional[Dict]:
        """删除Agent服务"""
        return self._delegate_to_http('delete_agent_service', agent_id, service_name)

    def list_agent_tools(self, agent_id: str) -> Optional[Dict]:
        """获取Agent工具列表"""
        return self._delegate_to_http('list_agent_tools', agent_id)

    def use_agent_tool(self, agent_id: str, tool_name: str, args: Dict) -> Optional[Dict]:
        """使用Agent工具"""
        return self._delegate_to_http('use_agent_tool', agent_id, tool_name, args)

    def get_agent_config(self, agent_id: str) -> Optional[Dict]:
        """获取Agent配置"""
        return self._delegate_to_http('get_agent_config', agent_id)

    def reset_agent_config(self, agent_id: str) -> Optional[Dict]:
        """重置Agent配置"""
        return self._delegate_to_http('reset_agent_config', agent_id)

    def get_agent_stats(self, agent_id: str) -> Optional[Dict]:
        """获取Agent统计"""
        return self._delegate_to_http('get_agent_stats', agent_id)

    def get_agent_health(self, agent_id: str) -> Optional[Dict]:
        """获取Agent健康状态"""
        return self._delegate_to_http('get_agent_health', agent_id)

    # ==================== 通用API ====================

    def get_service_by_name(self, service_name: str, agent_id: Optional[str] = None) -> Optional[Dict]:
        """通过名称获取服务"""
        return self._delegate_to_http('get_service_by_name', service_name, agent_id)
