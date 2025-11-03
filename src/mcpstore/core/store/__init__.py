# MCPStore 组合与对外导出（最新、单一路径架构）

from .composed_store import MCPStore
from .setup_manager import StoreSetupManager
from .client_manager import ClientManager

# 仅暴露权威 setup_store 入口（无历史兼容分支）
MCPStore.setup_store = staticmethod(StoreSetupManager.setup_store)

__all__ = ['MCPStore', 'ClientManager']
