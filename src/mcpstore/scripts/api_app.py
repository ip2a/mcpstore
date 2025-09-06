"""
MCPStore API Application Factory
Supports creating API applications using specified MCPStore instances
"""

import logging
import time
from contextlib import asynccontextmanager

from fastapi import Request, FastAPI
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from starlette.exceptions import HTTPException as StarletteHTTPException
from mcpstore.core.store import MCPStore

# 导入统一的异常处理器
from .api_exceptions import (
    mcpstore_exception_handler,
    validation_exception_handler,
    http_exception_handler,
    general_exception_handler
)

# Global store instance (set by MCPStore.start_api_server)
_global_store_instance: MCPStore = None

logger = logging.getLogger(__name__)

def get_store() -> MCPStore:
    """Get current MCPStore instance"""
    global _global_store_instance

    logger.info(f"get_store called, global instance: {_global_store_instance is not None}")
    if _global_store_instance is not None:
        logger.info(f"Global instance id: {id(_global_store_instance)}")

    if _global_store_instance is None:
        # If no global instance is set, create with default configuration
        logger.warning("No global store instance found, creating default store")
        _global_store_instance = MCPStore.setup_store()
    else:
        # Record the type of store being used
        is_data_space = _global_store_instance.is_using_data_space()
        workspace_dir = _global_store_instance.get_workspace_dir() if is_data_space else "default"
        logger.info(f"Using global store instance: data_space={is_data_space}, workspace={workspace_dir}")

    return _global_store_instance

def set_global_store(store: MCPStore):
    """Set the global MCPStore instance
    
    Args:
        store: The MCPStore instance to set as global
    """
    global _global_store_instance
    _global_store_instance = store
    logger.info(f"Global store instance updated: {id(store)}")

@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifecycle management"""
    store = get_store()
    
    logger.info("Initializing MCPStore API service...")
    
    if store.is_using_data_space():
        workspace_dir = store.get_workspace_dir()
        logger.info(f"Using data space: {workspace_dir}")
    else:
        logger.info("Using default configuration")
    
    # 检查编排器是否已经初始化
    try:
        # 检查关键组件是否已经启动
        if (hasattr(store.orchestrator, 'lifecycle_manager') and
            store.orchestrator.lifecycle_manager and
            store.orchestrator.lifecycle_manager.is_running):
            logger.info("Orchestrator already initialized, skipping setup")
        else:
            logger.info("Initializing orchestrator...")
            await store.orchestrator.setup()

        logger.info("MCPStore API service initialized successfully")
    except Exception as e:
        logger.error(f"Failed to setup orchestrator: {e}")
        raise
    
    yield  # 应用运行期间
    
    # 应用关闭时的清理
    logger.info("Shutting down MCPStore API service...")
    
    try:
        # 清理编排器资源
        await store.orchestrator.cleanup()
        logger.info("MCPStore API service shutdown completed")
    except Exception as e:
        logger.error(f"Error during shutdown: {e}")

def create_app() -> FastAPI:
    """
    创建FastAPI应用实例

    Returns:
        FastAPI: 配置好的应用实例
    """
    # 延迟获取store，避免在模块导入时触发
    # store = get_store()  # 移到lifespan中
    logger.info(f"Creating FastAPI app...")

    # 创建应用实例
    app = FastAPI(
        title="MCPStore API",
        description="MCPStore HTTP API Service",
        version="1.0.0",
        lifespan=lifespan
    )
    
    # 配置CORS
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
    
    # 导入并注册路由
    from .api import router
    app.include_router(router)
    
    # 注册统一的异常处理器
    app.add_exception_handler(RequestValidationError, validation_exception_handler)
    app.add_exception_handler(StarletteHTTPException, http_exception_handler)
    
    # 导入并注册MCPStore异常处理器
    from .api_exceptions import MCPStoreException
    app.add_exception_handler(MCPStoreException, mcpstore_exception_handler)
    
    # 注册通用异常处理器（最后注册，作为兜底）
    app.add_exception_handler(Exception, general_exception_handler)

    # 添加请求日志和性能监控中间件
    @app.middleware("http")
    async def log_requests_and_monitor(request: Request, call_next):
        """记录请求日志并监控性能"""
        start_time = time.time()
        
        # 增加活跃连接数
        try:
            store.for_store().increment_active_connections()
        except:
            pass  # 忽略监控错误
        
        try:
            response = await call_next(request)
            process_time = (time.time() - start_time) * 1000
            
            # 记录API调用
            try:
                store.for_store().record_api_call(process_time)
            except:
                pass  # 忽略监控错误

            # 只记录错误和较慢的请求
            if response.status_code >= 400 or process_time > 1000:
                logger.info(
                    f"{request.method} {request.url.path} - "
                    f"Status: {response.status_code}, Duration: {process_time:.2f}ms"
                )
            return response
        except Exception as e:
            process_time = (time.time() - start_time) * 1000
            logger.error(
                f"{request.method} {request.url.path} - "
                f"Error: {e}, Duration: {process_time:.2f}ms"
            )
            raise
        finally:
            # 减少活跃连接数
            try:
                store.for_store().decrement_active_connections()
            except:
                pass  # 忽略监控错误
    
    # 添加健康检查端点
    @app.get("/health")
    async def health_check():
        """健康检查端点"""
        try:
            store = get_store()
            workspace_info = None
            
            if store.is_using_data_space():
                workspace_info = {
                    "workspace_dir": store.get_workspace_dir(),
                    "mcp_config_path": store.config.json_path
                }
            
            return {
                "status": "healthy",
                "service": "MCPStore API",
                "version": "1.0.0",
                "timestamp": time.time(),
                "data_space": workspace_info
            }
        except Exception as e:
            logger.error(f"Health check failed: {e}")
            return JSONResponse(
                status_code=503,
                content={
                    "status": "unhealthy",
                    "service": "MCPStore API",
                    "error": str(e),
                    "timestamp": time.time()
                }
            )
    
    # 添加数据空间信息端点
    @app.get("/workspace/info")
    async def workspace_info():
        """获取工作空间信息"""
        try:
            store = get_store()
            
            if store.is_using_data_space():
                space_info = store.get_data_space_info()
                return {
                    "success": True,
                    "data": space_info,
                    "message": "Workspace information retrieved successfully"
                }
            else:
                return {
                    "success": True,
                    "data": {
                        "using_data_space": False,
                        "mcp_config_path": store.config.json_path,
                        "message": "Using default configuration"
                    },
                    "message": "Default workspace information"
                }
        except Exception as e:
            logger.error(f"Failed to get workspace info: {e}")
            return JSONResponse(
                status_code=500,
                content={
                    "success": False,
                    "message": f"Failed to get workspace info: {str(e)}",
                    "data": {}
                }
            )
    
    # 添加错误监控端点
    @app.get("/errors/stats")
    async def error_stats():
        """获取错误统计信息"""
        try:
            from .api_exceptions import error_monitor
            stats = error_monitor.get_error_stats()
            return {
                "success": True,
                "data": stats,
                "message": "Error statistics retrieved successfully"
            }
        except Exception as e:
            logger.error(f"Failed to get error stats: {e}")
            return JSONResponse(
                status_code=500,
                content={
                    "success": False,
                    "message": f"Failed to get error stats: {str(e)}",
                    "data": {}
                }
            )
    
    @app.post("/errors/clear")
    async def clear_error_stats():
        """清除错误统计信息"""
        try:
            from .api_exceptions import error_monitor
            error_monitor.clear_stats()
            return {
                "success": True,
                "data": {},
                "message": "Error statistics cleared successfully"
            }
        except Exception as e:
            logger.error(f"Failed to clear error stats: {e}")
            return JSONResponse(
                status_code=500,
                content={
                    "success": False,
                    "message": f"Failed to clear error stats: {str(e)}",
                    "data": {}
                }
            )
    
    return app

# 为了向后兼容，保留原有的app实例
app = create_app()
