"""
MCPStore API 服务 - 改进版
支持 CLI 启动时的 URL 前缀配置
"""

import logging
import os

# 导入应用工厂
from .api_app import create_app

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# 🆕 从环境变量读取 URL 前缀配置
url_prefix = os.environ.get("MCPSTORE_URL_PREFIX", "")

if url_prefix:
    logger.info(f"Creating app with URL prefix from environment: {url_prefix}")
else:
    logger.info("Creating app without URL prefix")

# 创建应用实例（CLI 启动时使用）
# store=None 表示使用默认配置
app = create_app(store=None, url_prefix=url_prefix)
