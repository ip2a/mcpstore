#!/usr/bin/env python3
"""
MCPStore直接调用API客户端
重构后的版本，直接调用MCPStore方法，不再使用HTTP API
"""

import json
import asyncio
from typing import Dict, List, Optional, Any
import logging
from .store_manager import get_store, is_store_initialized, initialize_store

class MCPStoreDirectAPI:
    """MCPStore直接调用API客户端"""
    
    def __init__(self):
        """初始化直接调用客户端"""
        self.logger = logging.getLogger(__name__)
        
        # 确保store已初始化
        if not is_store_initialized():
            initialize_store()
    
    def _get_store(self):
        """获取store实例"""
        return get_store()
    
    def _format_response(self, success: bool, data: Any = None, message: str = "") -> Dict:
        """格式化响应，保持与HTTP API相同的格式"""
        return {
            "success": success,
            "data": data,
            "message": message
        }
    
    def _handle_exception(self, e: Exception, operation: str) -> Dict:
        """处理异常并返回错误响应"""
        error_msg = f"{operation}失败: {str(e)}"
        self.logger.error(error_msg, exc_info=True)
        return self._format_response(False, None, error_msg)

    # _run_async方法已不再需要，因为MCPStore现在提供同步API
    
    # ==================== 连接测试 ====================
    
    def test_connection(self) -> bool:
        """测试连接 - 对应API: GET /for_store/health"""
        try:
            store = self._get_store()
            # 简单测试：尝试获取配置
            store.for_store().show_mcpconfig()
            return True
        except Exception as e:
            self.logger.error(f"连接测试失败: {e}")
            return False
    
    # ==================== Store级别服务管理 ====================
    
    def list_services(self) -> Optional[Dict]:
        """获取服务列表 - 对应API: GET /for_store/list_services"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            services = store.for_store().list_services()
            return self._format_response(True, services, "服务列表获取成功")
        except Exception as e:
            return self._handle_exception(e, "获取服务列表")
    
    def add_service(self, service_config: Dict) -> Optional[Dict]:
        """添加服务 - 对应API: POST /for_store/add_service"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            result = store.for_store().add_service(service_config)
            return self._format_response(True, result, "服务添加成功")
        except Exception as e:
            return self._handle_exception(e, "添加服务")
    
    def delete_service(self, service_name: str) -> Optional[Dict]:
        """删除服务 - 对应API: POST /for_store/delete_service"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            result = store.for_store().delete_service(service_name)
            return self._format_response(True, result, f"服务 {service_name} 删除成功")
        except Exception as e:
            return self._handle_exception(e, f"删除服务 {service_name}")
    
    def update_service(self, service_name: str, config: Dict) -> Optional[Dict]:
        """更新服务 - 对应API: POST /for_store/update_service"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            result = store.for_store().update_service(service_name, config)
            return self._format_response(True, result, f"服务 {service_name} 更新成功")
        except Exception as e:
            return self._handle_exception(e, f"更新服务 {service_name}")

    def restart_service(self, service_name: str) -> Optional[Dict]:
        """重启服务 - 对应API: POST /for_store/restart_service"""
        try:
            store = self._get_store()
            # restart_service可能不存在，先检查是否有这个方法
            if hasattr(store.for_store(), 'restart_service'):
                result = store.for_store().restart_service(service_name)
            else:
                # 如果没有restart_service，可以尝试重新添加服务
                result = store.for_store().update_service(service_name, {})
            return self._format_response(True, result, f"服务 {service_name} 重启成功")
        except Exception as e:
            return self._handle_exception(e, f"重启服务 {service_name}")

    def get_service_info(self, service_name: str) -> Optional[Dict]:
        """获取服务信息 - 对应API: POST /for_store/get_service_info"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            info = store.for_store().get_service_info(service_name)
            return self._format_response(True, info, f"服务 {service_name} 信息获取成功")
        except Exception as e:
            return self._handle_exception(e, f"获取服务 {service_name} 信息")

    def get_service_status(self, service_name: str) -> Optional[Dict]:
        """获取服务状态 - 对应API: POST /for_store/get_service_status"""
        try:
            store = self._get_store()
            # 使用get_service_info代替
            status = store.for_store().get_service_info(service_name)
            return self._format_response(True, status, f"服务 {service_name} 状态获取成功")
        except Exception as e:
            return self._handle_exception(e, f"获取服务 {service_name} 状态")

    def check_services(self) -> Optional[Dict]:
        """检查所有服务 - 对应API: GET /for_store/check_services"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            result = store.for_store().check_services()
            return self._format_response(True, result, "服务健康检查完成")
        except Exception as e:
            return self._handle_exception(e, "检查服务")
    
    # ==================== 批量操作 ====================
    
    def batch_add_services(self, services: List[Dict]) -> Optional[Dict]:
        """批量添加服务 - 对应API: POST /for_store/batch_add_services"""
        try:
            store = self._get_store()
            
            # 执行批量添加
            results = []
            succeeded = 0
            failed = 0
            
            for i, service in enumerate(services):
                try:
                    # 现在直接调用同步版本
                    result = store.for_store().add_service(service)
                    results.append({
                        "index": i,
                        "service": service,
                        "success": True,
                        "message": "Add operation succeeded"
                    })
                    succeeded += 1
                except Exception as e:
                    results.append({
                        "index": i,
                        "service": service,
                        "success": False,
                        "message": str(e)
                    })
                    failed += 1
            
            summary = {
                "total": len(services),
                "succeeded": succeeded,
                "failed": failed
            }
            
            data = {
                "results": results,
                "summary": summary
            }
            
            message = f"Batch add completed: {succeeded}/{len(services)} succeeded"
            return self._format_response(True, data, message)
            
        except Exception as e:
            return self._handle_exception(e, "批量添加服务")
    
    def batch_restart_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量重启服务 - 对应API: POST /for_store/batch_restart_services"""
        try:
            store = self._get_store()
            
            results = []
            succeeded = 0
            failed = 0
            
            for service_name in service_names:
                try:
                    # 现在直接调用同步版本
                    if hasattr(store.for_store(), 'restart_service'):
                        store.for_store().restart_service(service_name)
                    else:
                        # 如果没有restart_service，尝试更新服务
                        store.for_store().update_service(service_name, {})
                    results.append({
                        "name": service_name,
                        "success": True,
                        "message": "Restart succeeded"
                    })
                    succeeded += 1
                except Exception as e:
                    results.append({
                        "name": service_name,
                        "success": False,
                        "message": str(e)
                    })
                    failed += 1
            
            summary = {
                "total": len(service_names),
                "succeeded": succeeded,
                "failed": failed
            }
            
            data = {
                "results": results,
                "summary": summary
            }
            
            message = f"Batch restart completed: {succeeded}/{len(service_names)} succeeded"
            return self._format_response(True, data, message)
            
        except Exception as e:
            return self._handle_exception(e, "批量重启服务")
    
    def batch_delete_services(self, service_names: List[str]) -> Optional[Dict]:
        """批量删除服务 - 对应API: POST /for_store/batch_delete_services"""
        try:
            store = self._get_store()
            
            results = []
            succeeded = 0
            failed = 0
            
            for service_name in service_names:
                try:
                    # 现在直接调用同步版本
                    store.for_store().delete_service(service_name)
                    results.append({
                        "name": service_name,
                        "success": True,
                        "message": "Delete succeeded"
                    })
                    succeeded += 1
                except Exception as e:
                    results.append({
                        "name": service_name,
                        "success": False,
                        "message": str(e)
                    })
                    failed += 1
            
            summary = {
                "total": len(service_names),
                "succeeded": succeeded,
                "failed": failed
            }
            
            data = {
                "results": results,
                "summary": summary
            }
            
            message = f"Batch delete completed: {succeeded}/{len(service_names)} succeeded"
            return self._format_response(True, data, message)
            
        except Exception as e:
            return self._handle_exception(e, "批量删除服务")
    
    # ==================== 工具管理 ====================
    
    def list_tools(self) -> Optional[Dict]:
        """获取工具列表 - 对应API: GET /for_store/list_tools"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            tools = store.for_store().list_tools()
            return self._format_response(True, tools, "工具列表获取成功")
        except Exception as e:
            return self._handle_exception(e, "获取工具列表")

    def use_tool(self, tool_name: str, args: Dict) -> Optional[Dict]:
        """使用工具 - 对应API: POST /for_store/use_tool"""
        try:
            store = self._get_store()
            # 现在直接调用同步版本
            result = store.for_store().use_tool(tool_name, args)
            return self._format_response(True, result, f"工具 {tool_name} 执行成功")
        except Exception as e:
            return self._handle_exception(e, f"使用工具 {tool_name}")
    
    # ==================== 配置管理 ====================
    
    def get_config(self) -> Optional[Dict]:
        """获取配置 - 对应API: GET /for_store/get_config"""
        try:
            store = self._get_store()
            # get_config可能不存在，使用show_mcpconfig代替
            config = store.for_store().show_mcpconfig()
            return self._format_response(True, config, "配置获取成功")
        except Exception as e:
            return self._handle_exception(e, "获取配置")

    def show_mcpconfig(self) -> Optional[Dict]:
        """显示MCP配置 - 对应API: GET /for_store/show_mcpconfig"""
        try:
            store = self._get_store()
            # show_mcpconfig是同步方法
            config = store.for_store().show_mcpconfig()
            return self._format_response(True, config, "MCP配置获取成功")
        except Exception as e:
            return self._handle_exception(e, "获取MCP配置")

    def reset_config(self) -> Optional[Dict]:
        """重置配置 - 对应API: POST /for_store/reset_config"""
        try:
            store = self._get_store()
            # reset_config是同步方法
            result = store.for_store().reset_config()
            return self._format_response(True, result, "配置重置成功")
        except Exception as e:
            return self._handle_exception(e, "重置配置")


# 为了向后兼容，创建一个别名
MCPStoreAPI = MCPStoreDirectAPI
