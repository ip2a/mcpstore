"""
MCP Store Wiki - Local Studio Version (STDIO Mode)
用于 Claude Desktop / Claude Code 等本地 MCP 客户端

This version runs in STDIO mode for local development and testing.
"""
from fastmcp import FastMCP
from pydantic import Field
from typing import Annotated
import random
import logging
import sys
from datetime import datetime
from pathlib import Path

# Configure logging for stdio mode
LOG_DIR = Path("logs")
LOG_DIR.mkdir(exist_ok=True)

# Setup logging - for stdio mode, we log to file to avoid interfering with stdio communication
log_file = LOG_DIR / f"mcpstore_studio_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler(log_file, encoding='utf-8')
    ]
)

logger = logging.getLogger(__name__)

# Create FastMCP instance
mcp = FastMCP(
    name="MCPStoreWiki",
    instructions="MCP Store Wiki service providing documentation and weather tools"
)

@mcp.tool()
def get_current_weather(
    query: Annotated[str, Field(description="城市名称，例如：北京、上海、广州")]
) -> str:
    """获取指定城市的当前天气信息，包括温度和天气状况"""
    logger.info(f"获取天气信息请求: city={query}")

    weather_conditions = ["晴天", "多云", "小雨", "阴天", "雪"]
    temperature = random.randint(-5, 35)
    condition = random.choice(weather_conditions)
    humidity = random.randint(30, 90)

    result = f"{query}当前天气：{condition}，温度{temperature}°C，湿度{humidity}%"
    logger.info(f"天气信息返回: {result}")

    return result


@mcp.tool()
def get_mcpstore_docs() -> str:
    """获取 MCPStore 的文档链接"""
    logger.info("获取文档链接请求")
    result = "MCPStore 文档地址：https://doc.mcpstore.wiki/"
    logger.info(f"文档链接返回: {result}")
    return result


@mcp.tool()
def get_weather_forecast(
    city: Annotated[str, Field(description="城市名称")],
    days: Annotated[int, Field(description="预报天数 (1-7)", ge=1, le=7)] = 3
) -> str:
    """获取未来几天的天气预报"""
    logger.info(f"获取天气预报请求: city={city}, days={days}")

    weather_conditions = ["晴天", "多云", "小雨", "阴天", "雪"]
    forecast = [f"{city}未来{days}天天气预报：\n"]

    for day in range(1, days + 1):
        temperature = random.randint(-5, 35)
        condition = random.choice(weather_conditions)
        forecast.append(f"第{day}天：{condition}，温度{temperature}°C")

    result = "\n".join(forecast)
    logger.info(f"天气预报返回: {result}")

    return result


@mcp.resource("mcpstore://wiki/introduction")
def get_introduction() -> str:
    """MCPStore 项目简介"""
    return """
# MCPStore 项目简介

MCPStore 是一个强大的 Model Context Protocol (MCP) 服务管理平台。

## 核心特性
- 🚀 快速部署 MCP 服务
- 🔧 灵活的工具管理
- 📊 实时健康监控
- 🔄 双向同步机制
- 💾 Redis 缓存支持

## 文档资源
- 官方文档：https://doc.mcpstore.wiki/
- GitHub：https://github.com/MCPStore/mcpstore
    """


@mcp.resource("mcpstore://wiki/quickstart")
def get_quickstart() -> str:
    """MCPStore 快速开始指南"""
    return """
# MCPStore 快速开始

## 安装
```bash
pip install mcpstore
```

## 基本使用
```python
from mcpstore import MCPHub

# 创建 MCP Hub 实例
hub = MCPHub()

# 启动服务
hub.start()
```

## 更多信息
访问 https://doc.mcpstore.wiki/ 获取完整文档
    """


@mcp.prompt()
def explain_mcp(
    topic: Annotated[str, Field(description="需要解释的 MCP 主题")] = "overview"
) -> str:
    """生成关于 MCP 概念的解释提示词"""
    prompts = {
        "overview": "请解释什么是 Model Context Protocol (MCP)，它的核心价值是什么？",
        "tools": "请详细说明 MCP 中的工具（tools）概念，以及如何使用它们？",
        "resources": "请解释 MCP 中的资源（resources）是什么，它们与工具有什么区别？",
        "prompts": "请说明 MCP 中的提示词（prompts）功能，以及它们的使用场景？"
    }

    return prompts.get(topic, f"请解释 MCP 中关于 {topic} 的概念")


if __name__ == "__main__":
    logger.info("=" * 60)
    logger.info("MCPStore Wiki Studio - 启动 (STDIO 模式)")
    logger.info(f"日志文件: {log_file.absolute()}")
    logger.info("=" * 60)

    try:
        # Run in STDIO mode for Claude Desktop integration
        logger.info("服务启动: transport=stdio")
        mcp.run(transport="stdio")
    except Exception as e:
        logger.error(f"服务启动失败: {e}", exc_info=True)
        raise
