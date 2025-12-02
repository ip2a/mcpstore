# MCPStore API Service
FROM python:3.11-slim

# Set environment variables
ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install uv
RUN pip install --no-cache-dir uv

# Set working directory
WORKDIR /app

# Copy project files
COPY ../../src ./src
COPY ../../pyproject.toml .
COPY ../../README.md .

# Copy startup script
COPY start_api.py .

# Install project dependencies using uv
RUN uv pip install --system -e .

# Create data directories
RUN mkdir -p /app/data /app/config /app/logs

# Set script permissions
RUN chmod +x start_api.py

# Expose port
EXPOSE 18200

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
  CMD curl -f http://localhost:18200/health || exit 1

# Start command
CMD ["python", "start_api.py"]