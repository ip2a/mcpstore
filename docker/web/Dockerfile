# Vue.js Multi-stage Build
FROM node:18-alpine AS builder

# Set working directory
WORKDIR /build

# Install pnpm (recommended) or use npm
RUN npm install -g pnpm

# Copy package.json and lock files
COPY vue/package.json vue/pnpm-lock.yaml ./

# Install dependencies
RUN pnpm install --frozen-lockfile

# Copy source code
COPY vue/ .

# Build application
RUN pnpm build

# Production stage - Serve static files with nginx
FROM nginx:alpine

# Install curl for health checks
RUN apk add --no-cache curl

# Copy build artifacts
COPY --from=builder /build/dist /usr/share/nginx/html

# Copy nginx configuration
RUN echo 'server { \
    listen 5177; \
    server_name _; \
    root /usr/share/nginx/html; \
    index index.html; \
    \
    location / { \
        try_files $$uri $$uri/ /index.html; \
    } \
    \
    # Support reverse proxy for API requests to local API service \
    location /api/ { \
        proxy_pass http://host.docker.internal:18200/; \
        proxy_set_header Host $$host; \
        proxy_set_header X-Real-IP $$remote_addr; \
        proxy_set_header X-Forwarded-For $$proxy_add_x_forwarded_for; \
        proxy_set_header X-Forwarded-Proto $$scheme; \
        proxy_http_version 1.1; \
        proxy_set_header Upgrade $$http_upgrade; \
        proxy_set_header Connection "upgrade"; \
    } \
    \
    # Static resource caching \
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)$$ { \
        expires 1y; \
        add_header Cache-Control "public, immutable"; \
    } \
}' > /etc/nginx/conf.d/default.conf

# Expose port
EXPOSE 5177

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:5177/ || exit 1

# Start nginx
CMD ["nginx", "-g", "daemon off;"]
