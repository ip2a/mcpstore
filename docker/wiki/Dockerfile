# MCP Wiki Service
FROM python:3.11-slim

# Set environment variables
ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1 \
    LOG_LEVEL=INFO

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install uv
RUN pip install --no-cache-dir uv

# Set working directory
WORKDIR /app

# Copy requirements file
COPY requirements.txt .

# Install dependencies using uv
RUN uv pip install --system -r requirements.txt

# Copy wiki service file
COPY ../../wiki/mcp_service_wiki.py .

# Create logs directory
RUN mkdir -p /app/logs

# Set script permissions
RUN chmod +x mcp_service_wiki.py

# Expose port
EXPOSE 21923

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
  CMD curl -f http://localhost:21923/mcp || exit 1

# Start command
CMD ["python", "mcp_service_wiki.py"]