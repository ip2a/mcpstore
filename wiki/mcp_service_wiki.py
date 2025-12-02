from fastmcp import FastMCP
from fastmcp.server.dependencies import get_http_request, get_http_headers
from pydantic import Field
from typing import Annotated
import random
import logging
import sys
import threading
import time
import glob
import os
from datetime import datetime, timedelta
from pathlib import Path
import json

# Log persistence configuration
LOG_DIR = Path("logs")
LOG_DIR.mkdir(exist_ok=True)
MAX_LOG_SIZE = 5 * 1024 * 1024  # 5MB = 5 * 1024 * 1024 bytes
LOG_CLEANUP_DAYS = 30  # Keep logs for 30 days
LOG_CLEANUP_INTERVAL = 86400  # Check cleanup once per day = 86400 seconds

class SizeBasedRotatingLogHandler:
    """Size-based log rotation handler"""

    def __init__(self, log_dir: Path, max_size: int, cleanup_days: int, cleanup_interval: int):
        self.log_dir = log_dir
        self.max_size = max_size
        self.cleanup_days = cleanup_days
        self.cleanup_interval = cleanup_interval
        self.current_handler = None
        self.current_log_file = None
        self.log_counter = 1
        self.last_cleanup = time.time()
        self.lock = threading.Lock()

        # Create initial log file
        self._rotate_log()

        # Start cleanup thread
        self.cleanup_thread = threading.Thread(target=self._cleanup_worker, daemon=True)
        self.cleanup_thread.start()

    def _get_log_filename(self):
        """Generate log filename with date and sequence number"""
        date_str = datetime.now().strftime("%Y%m%d")
        return self.log_dir / f"mcpstorewiki_{date_str}_{self.log_counter:03d}.log"

    def _get_current_log_size(self):
        """Get current log file size"""
        if self.current_log_file and self.current_log_file.exists():
            return self.current_log_file.stat().st_size
        return 0

    def _should_rotate(self):
        """Check if log rotation is needed"""
        return self._get_current_log_size() >= self.max_size

    def _rotate_log(self):
        """Rotate log file"""
        with self.lock:
            # Close current handler
            if self.current_handler:
                logger.removeHandler(self.current_handler)
                self.current_handler.close()

            # Create new file handler
            self.current_log_file = self._get_log_filename()
            self.current_handler = logging.FileHandler(self.current_log_file, encoding='utf-8')
            self.current_handler.setFormatter(
                logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
            )
            logger.addHandler(self.current_handler)

            logger.info(f"📁 Log rotation: New log file {self.current_log_file} (max size: {self.max_size / 1024 / 1024:.1f}MB)")
            self.log_counter += 1

    def check_and_rotate(self):
        """Check file size and rotate if needed"""
        if self._should_rotate():
            self._rotate_log()

    def _cleanup_old_logs(self):
        """Clean up old log files"""
        cutoff_time = datetime.now() - timedelta(days=self.cleanup_days)
        pattern = str(self.log_dir / "mcpstorewiki_*.log")

        cleaned_count = 0
        for log_file in glob.glob(pattern):
            file_path = Path(log_file)
            if file_path.stat().st_mtime < cutoff_time.timestamp():
                try:
                    file_path.unlink()
                    cleaned_count += 1
                    logger.info(f"🗑️ Cleaned old log: {file_path}")
                except Exception as e:
                    logger.error(f"❌ Failed to clean log {file_path}: {e}")

        if cleaned_count > 0:
            logger.info(f"🧹 Log cleanup completed: Deleted {cleaned_count} old log files")
        else:
            logger.info("🧹 Log cleanup completed: No files to clean")

    def _cleanup_worker(self):
        """Log cleanup worker thread"""
        while True:
            time.sleep(self.cleanup_interval)
            self._cleanup_old_logs()

# Configure basic logging
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout)
    ]
)

logger = logging.getLogger(__name__)

# Start log persistence
log_handler = SizeBasedRotatingLogHandler(LOG_DIR, MAX_LOG_SIZE, LOG_CLEANUP_DAYS, LOG_CLEANUP_INTERVAL)

# Create FastMCP instance
mcp = FastMCP(
    name="WeatherService"
)

# Enhanced request logging function
def log_request_info(endpoint_name, **kwargs):
    """Log key information: time, client IP, input parameters (concise logging)"""
    request_time = datetime.now()

    # Get client IP
    client_ip = "unknown"
    try:
        request = get_http_request()
        if request and request.client:
            client_ip = request.client.host

        # Try to get real IP from headers (handle proxy cases)
        headers = get_http_headers()
        if headers:
            # Check common proxy headers
            for header_name in ['X-Forwarded-For', 'X-Real-IP', 'CF-Connecting-IP']:
                if header_name in headers:
                    forwarded_ip = headers[header_name].split(',')[0].strip()
                    if forwarded_ip:
                        client_ip = forwarded_ip
                        break
    except Exception as e:
        logger.debug(f"Failed to get client IP: {e}")

    # Concise request start log
    logger.info(f"request_start endpoint={endpoint_name} time={request_time.isoformat()} ip={client_ip}")

    # Brief parameter recording (avoid too long)
    if kwargs:
        safe_kwargs = {}
        for key, value in kwargs.items():
            if isinstance(value, str) and len(value) > 200:
                safe_kwargs[key] = value[:200] + "..."
            else:
                safe_kwargs[key] = value
        logger.info(f"request_params endpoint={endpoint_name} params={safe_kwargs}")

    # Record HTTP request details (if available)
    try:
        request = get_http_request()
        if request:
            logger.info(f"request_http endpoint={endpoint_name} path={request.url.path} method={request.method}")

        headers = get_http_headers()
        if headers:
            header_summary = {}
            for header_name in ['User-Agent', 'Content-Type', 'X-Forwarded-For']:
                if header_name in headers:
                    header_summary[header_name] = headers[header_name]
            if header_summary:
                logger.info(f"request_headers endpoint={endpoint_name} headers={header_summary}")
    except Exception as e:
        logger.debug(f"Failed to get HTTP request details: {e}")

    # Check log file size and rotate if needed
    log_handler.check_and_rotate()

    return {
        'timestamp': request_time.isoformat(),
        'client_ip': client_ip,
        'endpoint': endpoint_name,
        'params': kwargs
    }

@mcp.tool()
def get_current_weather(
    query: Annotated[str, Field(description="City name to query weather, e.g., Beijing, Shanghai, Guangzhou")]
) -> str:
    """Get current weather information for specified city, including temperature and weather conditions"""
    # Log detailed request information
    request_info = log_request_info("get_current_weather", query=query)

    weather_conditions = ["Sunny", "Cloudy", "Light Rain", "Overcast", "Snow"]
    temperature = random.randint(-5, 35)
    condition = random.choice(weather_conditions)
    humidity = random.randint(30, 90)

    result = f"{query} current weather: {condition}, temperature {temperature}°C, humidity {humidity}%"

    # Brief completion log
    end_time = datetime.now()
    logger.info(f"tool_done name=get_current_weather time={end_time.isoformat()} ip={request_info['client_ip']} result={result}")

    # Check log file size and rotate if needed
    log_handler.check_and_rotate()

    return result


@mcp.tool()
def get_mcpstore_docs() -> str:
    """Return mcpstore documentation URL"""
    # Log detailed request information
    request_info = log_request_info("get_mcpstore_docs")

    result = "https://doc.mcpstore.wiki/"

    # Brief completion log
    end_time = datetime.now()
    logger.info(f"tool_done name=get_mcpstore_docs time={end_time.isoformat()} ip={request_info['client_ip']} url={result}")

    # Check log file size and rotate if needed
    log_handler.check_and_rotate()

    return result


if __name__ == "__main__":
    logger.info(f"service_start transport=streamable-http host=0.0.0.0 port=21923 path=/mcp")
    logger.info(
        f"log_config dir={LOG_DIR.absolute()} max_mb={MAX_LOG_SIZE / 1024 / 1024:.1f}"
        f" cleanup_days={LOG_CLEANUP_DAYS} cleanup_interval_sec={LOG_CLEANUP_INTERVAL}"
    )

    try:
        logger.info("service_boot")
        mcp.run(
            transport="streamable-http",
            host="0.0.0.0",
            port=21923,
            path="/mcp"
        )
    except Exception as e:
        logger.error(f"service_start_failed error={e} type={type(e).__name__}")
        import traceback
        logger.error(f"trace={traceback.format_exc()}")
        raise
