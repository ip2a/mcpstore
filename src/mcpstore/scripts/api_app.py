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

    logger.debug(f"get_store called, global instance: {_global_store_instance is not None}")
    if _global_store_instance is not None:
        logger.debug(f"Global instance id: {id(_global_store_instance)}")

    if _global_store_instance is None:
        # If no global instance is set, create with default configuration
        logger.warning("No global store instance found, creating default store")
        _global_store_instance = MCPStore.setup_store()
    else:
        # Record the type of store being used
        is_data_space = _global_store_instance.is_using_data_space()
        workspace_dir = _global_store_instance.get_workspace_dir() if is_data_space else "default"
        logger.debug(f"Using global store instance: data_space={is_data_space}, workspace={workspace_dir}")

    return _global_store_instance

def set_global_store(store: MCPStore):
    """Set the global MCPStore instance
    
    Args:
        store: The MCPStore instance to set as global
    """
    global _global_store_instance
    _global_store_instance = store
    logger.debug(f"Global store instance updated: {id(store)}")

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
    
    # 记录应用启动时间（用于health check）
    app._start_time = time.time()
    
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
    
    # 添加 API 文档入口
    @app.get("/doc")
    async def api_documentation():
        """
        API 文档入口
        
        返回所有可用的 API 文档链接
        """
        from mcpstore.core.models import ResponseBuilder
        
        return ResponseBuilder.success(
            message="MCPStore API Documentation",
            data={
                "documentation": {
                    "swagger_ui": {
                        "url": "/docs",
                        "description": "Swagger UI - 交互式 API 文档，可以直接测试接口"
                    },
                    "redoc": {
                        "url": "/redoc",
                        "description": "ReDoc - 更美观的 API 文档展示"
                    },
                    "openapi_json": {
                        "url": "/openapi.json",
                        "description": "OpenAPI 规范文件（JSON 格式）"
                    }
                },
                "quick_links": {
                    "api_root": "/",
                    "health_check": "/health",
                    "route_info": "查看根路径 / 获取详细的路由统计信息"
                }
            }
        )
    
    # 添加健康检查端点
    @app.get("/health")
    async def health_check():
        """健康检查端点"""
        from mcpstore.core.models import ResponseBuilder, ErrorCode
        from datetime import datetime
        
        try:
            store = get_store()
            
            # 统计服务数量
            try:
                context = store.for_store()
                services = context.list_services()
                services_count = len(services)
                agents_count = len(store.list_all_agents()) if hasattr(store, 'list_all_agents') else 0
            except:
                services_count = 0
                agents_count = 0
            
            # 计算运行时间
            uptime_seconds = int(time.time() - getattr(app, '_start_time', time.time()))
            
            return ResponseBuilder.success(
                message="System is healthy",
                data={
                    "status": "healthy",
                    "uptime_seconds": uptime_seconds,
                    "services_count": services_count,
                    "agents_count": agents_count
                }
            )
        except Exception as e:
            logger.error(f"Health check failed: {e}")
            response = ResponseBuilder.error(
                code=ErrorCode.INTERNAL_ERROR,
                message="Health check failed",
                details={"error": str(e)}
            )
            return JSONResponse(
                status_code=503,
                content=response.dict(exclude_none=True)
            )
    
    return app

# 为了向后兼容，保留原有的app实例
app = create_app()
