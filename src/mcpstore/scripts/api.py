"""
MCPStore API 主路由注册文件
整合所有子模块的路由，提供统一的API入口

重构说明：
- 原来的2391行api.py文件已按功能模块拆分为：
  * api_models.py - 所有响应模型
  * api_decorators.py - 装饰器和工具函数
  * api_store.py - Store级别路由
  * api_agent.py - Agent级别路由
  * api_monitoring.py - 监控相关路由
- 本文件负责统一注册所有子路由，保持API接口的兼容性
"""

from fastapi import APIRouter

from .api_agent import agent_router
from .api_monitoring import monitoring_router
# 导入所有子路由模块
from .api_store import store_router

# 导入依赖注入函数（保持兼容性）

# 创建主路由器
router = APIRouter()

# 注册所有子路由
# Store级别操作路由
router.include_router(store_router, tags=["Store Operations"])

# Agent级别操作路由
router.include_router(agent_router, tags=["Agent Operations"])

# 监控和统计路由
router.include_router(monitoring_router, tags=["Monitoring & Statistics"])

# 保持向后兼容性 - 导出常用的函数和类
# 这样现有的导入语句仍然可以正常工作

# 路由统计信息（用于调试）
def get_route_info():
    """获取路由统计信息"""
    total_routes = len(router.routes)
    store_routes = len(store_router.routes)
    agent_routes = len(agent_router.routes)
    monitoring_routes = len(monitoring_router.routes)

    return {
        "total_routes": total_routes,
        "store_routes": store_routes,
        "agent_routes": agent_routes,
        "monitoring_routes": monitoring_routes,
        "modules": {
            "api_store.py": f"{store_routes} routes",
            "api_agent.py": f"{agent_routes} routes",
            "api_monitoring.py": f"{monitoring_routes} routes"
        }
    }

# 健康检查端点（简单的根路径检查）
@router.get("/", tags=["System"])
async def api_root():
    """API根路径 - 系统信息"""
    from mcpstore.core.models.common import APIResponse

    route_info = get_route_info()

    return APIResponse(
        success=True,
        data={
            "message": "MCPStore API Server",
            "version": "0.6.0",
            "status": "running",
            "routes": route_info,
            "documentation": "/docs",
            "openapi": "/openapi.json"
        },
        message="MCPStore API is running successfully"
    )
