# MkDocs Multi-stage Build
FROM python:3.11-slim

# Set working directory
WORKDIR /app

# Install uv
RUN pip install --no-cache-dir uv

# Copy requirements file
COPY ../docs/requirements.txt .

# Install dependencies using uv
RUN uv pip install --system -r requirements.txt

# Copy documentation files
COPY ../docs/mkdocs.yml ./mkdocs.yml
COPY ../docs/docs ./docs
COPY ../docs/assets ./assets

# Create logs directory
RUN mkdir -p /app/logs

# Set environment variables
ENV PYTHONUNBUFFERED=1
ENV MKDOCS_CONFIG=/app/mkdocs.yml
ENV LOG_LEVEL=INFO

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8000/ || exit 1

# Start command
CMD ["mkdocs", "serve", "--host", "0.0.0.0", "--port", "8000", "--config-file", "/app/mkdocs.yml"]