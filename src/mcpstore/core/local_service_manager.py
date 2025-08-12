"""
Local MCP Service Manager (Refactored)
Now uses FastMCP for all local service management, providing backward compatibility.

🔧 重构说明：
- LocalServiceManager现在使用FastMCP作为底层实现
- 所有进程管理、环境变量处理都委托给FastMCP
- 保持向后兼容的API接口
- 删除了300+行的重复代码，现在只有配置规范化逻辑
"""

import logging
from typing import Dict, Optional, Tuple, Any
from pathlib import Path

# Import the new FastMCP-based implementation
from .local_service_adapter import (
    LocalServiceManagerAdapter,
    LocalServiceProcess,
    get_local_service_manager as get_adapter,
    set_local_service_manager_work_dir
)

logger = logging.getLogger(__name__)

# 向后兼容性：重新导出适配器类作为LocalServiceManager
LocalServiceManager = LocalServiceManagerAdapter

# 向后兼容性：重新导出LocalServiceProcess（已在adapter中定义）
# LocalServiceProcess已在local_service_adapter.py中定义
def get_local_service_manager() -> LocalServiceManagerAdapter:
    """
    获取全局本地服务管理器实例

    🔧 重构说明：现在返回FastMCP适配器实例，提供相同的API但使用FastMCP实现

    Returns:
        LocalServiceManagerAdapter: 适配器实例（兼容原LocalServiceManager接口）
    """
    return get_adapter()
# 🔧 重构完成：所有LocalServiceManager的功能现在都通过FastMCP适配器提供
# 原来的300+行代码已经被FastMCP的标准实现替代
# 用户可以继续使用相同的API，但底层使用FastMCP处理所有进程管理和环境变量
# LocalServiceManager现在使用FastMCP适配器实现
# 所有本地服务管理功能委托给FastMCP处理
