# MCPStore All-in-One Dockerfile

# Stage 1: Build frontend assets
FROM node:18-alpine AS frontend_builder

WORKDIR /build

COPY vue/package*.json ./
RUN npm config set registry https://registry.npmmirror.com && \
    npm ci --production=false

COPY vue/ .
RUN npm run build

# Stage 2: Runtime image with backend API and Nginx
FROM python:3.10-slim

ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    nginx \
    supervisor \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir mcpstore

RUN mkdir -p /app/data /app/config /app/logs

COPY --from=frontend_builder /build/dist /usr/share/nginx/html

RUN cat <<'EOF' > /etc/nginx/conf.d/default.conf
server {
    listen 80;
    server_name _;
    root /usr/share/nginx/html;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://127.0.0.1:18200/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
EOF

RUN cat <<'EOF' > /app/start_backend.py
from mcpstore import MCPStore


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    store.start_api_server(
        host="0.0.0.0",
        port=18200,
        log_level="info",
        reload=False,
        auto_open_browser=False
    )


if __name__ == "__main__":
    main()
EOF

RUN chmod +x /app/start_backend.py

RUN cat <<'EOF' > /etc/supervisor/conf.d/mcpstore.conf
[supervisord]
nodaemon=true

[program:backend]
command=python /app/start_backend.py
priority=10
autostart=true
autorestart=true

[program:nginx]
command=/usr/sbin/nginx -g "daemon off;"
priority=20
autostart=true
autorestart=true
EOF

EXPOSE 80

CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/mcpstore.conf"]

