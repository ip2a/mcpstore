from fastmcp import FastMCP
from fastmcp.server.dependencies import get_http_request, get_http_headers
from pydantic import Field
from typing import Annotated
import random
import logging
import sys
from pathlib import Path
from logging.handlers import RotatingFileHandler

# --- Configuration ---
LOG_DIR = Path("logs")
LOG_DIR.mkdir(exist_ok=True)
LOG_FILE = LOG_DIR / "mcp_service.log"
MAX_LOG_SIZE = 5 * 1024 * 1024  # 5MB
BACKUP_COUNT = 30  # Keep last 30 log files (replaces complex time-based cleanup)

# --- Logging Setup ---
# Use standard RotatingFileHandler for robust size-based rotation and cleanup
file_handler = RotatingFileHandler(
    LOG_FILE,
    maxBytes=MAX_LOG_SIZE,
    backupCount=BACKUP_COUNT,
    encoding='utf-8'
)
console_handler = logging.StreamHandler(sys.stdout)

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[file_handler, console_handler]
)
logger = logging.getLogger("WeatherMCP")

# --- FastMCP Instance ---
mcp = FastMCP(name="WeatherService")


# --- Helper: Get Client IP ---
def get_client_ip() -> str:
    """Helper to extract client IP from request or headers"""
    try:
        req = get_http_request()
        if req and req.client:
            return req.client.host
        headers = get_http_headers() or {}
        # Check common proxy headers
        for h in ['X-Forwarded-For', 'X-Real-IP']:
            if h in headers:
                return headers[h].split(',')[0].strip()
    except Exception:
        pass
    return "unknown"


# --- Tools ---

@mcp.tool()
def get_current_weather(
        query: Annotated[str, Field(description="City name to query weather")]
) -> str:
    """Get current weather information"""
    # 1. Concise Request Log
    client_ip = get_client_ip()
    logger.info(f"REQ [get_current_weather] IP={client_ip} | Query='{query}'")

    # Business Logic
    weather_conditions = ["Sunny", "Cloudy", "Light Rain", "Overcast", "Snow"]
    temperature = random.randint(-5, 35)
    condition = random.choice(weather_conditions)
    humidity = random.randint(30, 90)

    result = f"{query} current weather: {condition}, temp {temperature}°C, humidity {humidity}%"

    # 2. Concise Response Log
    logger.info(f"RES [get_current_weather] Result='{result}'")
    return result


@mcp.tool()
def get_mcpstore_docs() -> str:
    """Return mcpstore documentation URL"""
    client_ip = get_client_ip()
    logger.info(f"REQ [get_mcpstore_docs] IP={client_ip}")

    result = "https://doc.mcpstore.wiki/"

    logger.info(f"RES [get_mcpstore_docs] URL='{result}'")
    return result


if __name__ == "__main__":
    logger.info(f"SERVICE START | Log Dir: {LOG_DIR.absolute()}")

    try:
        mcp.run(transport="streamable-http", host="0.0.0.0", port=21923, path="/mcp")
    except Exception as e:
        logger.error(f"SERVICE CRASH: {e}", exc_info=True)