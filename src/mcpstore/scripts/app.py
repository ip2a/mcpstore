"""
MCPStore API 服务 - 已弃用
请使用 api_app.py 中的 create_app() 函数

此文件仅保留用于向后兼容，将在未来版本中移除
"""

import logging
import os

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

# 导入新的应用工厂
from .api_app import create_app

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# 为了向后兼容，创建应用实例
# 注意：这个文件已被弃用，请使用 api_app.py
logger.warning("app.py is deprecated. Please use api_app.create_app() instead.")

# 创建应用实例（委托给新的工厂函数）
app = create_app()

# 添加弃用警告的中间件
@app.middleware("http")
async def deprecation_warning(request, call_next):
    """添加弃用警告"""
    if request.url.path.startswith("/docs") or request.url.path.startswith("/openapi"):
        # API 文档路径，添加警告头部
        response = await call_next(request)
        response.headers["X-Deprecation-Warning"] = "app.py is deprecated. Please migrate to api_app.create_app()"
        return response
    return await call_next(request)
