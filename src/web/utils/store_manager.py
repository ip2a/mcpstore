#!/usr/bin/env python3
"""
MCPStore管理模块
负责初始化和管理MCPStore实例，提供统一的store访问接口
"""

import os
import sys
import logging
from typing import Optional

# 添加MCPStore路径
sys.path.append(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(__file__))), 'mcpstore'))

from mcpstore.core.store import MCPStore

class StoreManager:
    """MCPStore管理器"""
    
    _instance: Optional['StoreManager'] = None
    _store: Optional[MCPStore] = None
    
    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance
    
    def __init__(self):
        if not hasattr(self, '_initialized'):
            self._initialized = True
            self._store = None
            self._logger = logging.getLogger(__name__)
    
    def initialize_store(self, config_path: Optional[str] = None) -> MCPStore:
        """
        初始化MCPStore实例

        Args:
            config_path: 配置文件路径，如果为None则使用默认路径（暂时未使用）

        Returns:
            MCPStore实例
        """
        if self._store is None:
            try:
                self._logger.info("初始化MCPStore...")

                # 使用MCPStore的静态方法初始化
                # 这会自动处理配置文件路径和所有必要的组件
                self._store = MCPStore.setup_store()

                self._logger.info("MCPStore初始化成功")

            except Exception as e:
                self._logger.error(f"MCPStore初始化失败: {e}")
                raise

        return self._store
    
    def get_store(self) -> MCPStore:
        """
        获取MCPStore实例
        
        Returns:
            MCPStore实例
            
        Raises:
            RuntimeError: 如果store未初始化
        """
        if self._store is None:
            raise RuntimeError("MCPStore未初始化，请先调用initialize_store()")
        
        return self._store
    
    def reset_store(self):
        """重置store实例"""
        if self._store:
            try:
                # 清理资源
                self._store = None
                self._logger.info("MCPStore实例已重置")
            except Exception as e:
                self._logger.error(f"重置MCPStore时出错: {e}")
    
    def is_initialized(self) -> bool:
        """检查store是否已初始化"""
        return self._store is not None


# 全局store管理器实例
store_manager = StoreManager()


def get_store() -> MCPStore:
    """
    获取全局MCPStore实例的便捷函数
    
    Returns:
        MCPStore实例
    """
    return store_manager.get_store()


def initialize_store(config_path: Optional[str] = None) -> MCPStore:
    """
    初始化全局MCPStore实例的便捷函数
    
    Args:
        config_path: 配置文件路径
        
    Returns:
        MCPStore实例
    """
    return store_manager.initialize_store(config_path)


def is_store_initialized() -> bool:
    """
    检查全局store是否已初始化的便捷函数
    
    Returns:
        是否已初始化
    """
    return store_manager.is_initialized()


class StoreContextManager:
    """Store上下文管理器，用于确保store在使用前已初始化"""
    
    def __init__(self, config_path: Optional[str] = None):
        self.config_path = config_path
        self.store = None
    
    def __enter__(self) -> MCPStore:
        if not is_store_initialized():
            self.store = initialize_store(self.config_path)
        else:
            self.store = get_store()
        return self.store
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        # 不在这里清理store，保持全局实例
        pass


def with_store(func):
    """
    装饰器：确保函数执行时store已初始化
    
    Usage:
        @with_store
        def my_function():
            store = get_store()
            # 使用store...
    """
    def wrapper(*args, **kwargs):
        if not is_store_initialized():
            initialize_store()
        return func(*args, **kwargs)
    return wrapper


# 为了向后兼容，提供一些常用的store方法快捷访问
def get_store_services():
    """获取store级别的服务列表"""
    store = get_store()
    return store.for_store().list_services()


def get_store_tools():
    """获取store级别的工具列表"""
    store = get_store()
    return store.for_store().list_tools()


def add_store_service(service_config: dict):
    """添加store级别的服务"""
    store = get_store()
    return store.for_store().add_service(service_config)


def get_mcp_config():
    """获取MCP配置"""
    store = get_store()
    return store.for_store().show_mcpconfig()


def update_mcp_config(config: dict):
    """更新MCP配置"""
    store = get_store()
    return store.for_store().update_config(config)


if __name__ == "__main__":
    # 测试代码
    print("测试MCPStore管理器...")
    
    try:
        # 初始化store
        store = initialize_store()
        print(f"✅ Store初始化成功: {type(store)}")
        
        # 测试获取store
        store2 = get_store()
        print(f"✅ 获取store成功: {store is store2}")
        
        # 测试上下文管理器
        with StoreContextManager() as store3:
            print(f"✅ 上下文管理器: {store is store3}")
        
        # 测试便捷方法
        services = get_store_services()
        print(f"✅ 获取服务列表: {len(services) if services else 0} 个服务")
        
        print("🎉 所有测试通过！")
        
    except Exception as e:
        print(f"❌ 测试失败: {e}")
        import traceback
        traceback.print_exc()
