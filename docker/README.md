# MCPStore Docker Service Deployment

## 🏗️ Service Architecture

MCPStore now adopts a microservices architecture, containing 4 independent services:

- **doc**: MkDocs Documentation Service (Port: 8000)
- **web**: Vue.js Frontend Service (Port: 5177)
- **api**: MCPStore API Backend Service (Port: 18200)
- **wiki**: MCP Wiki Service (Port: 21923)

## 🚀 Quick Start

### Start Individual Services

Each service has its own Docker Compose configuration:

```bash
# Start documentation service
cd docker/doc && docker-compose up -d

# Start frontend service
cd docker/web && docker-compose up -d

# Start API service
cd docker/api && docker-compose up -d

# Start Wiki service
cd docker/wiki && docker-compose up -d
```

### Batch Startup Scripts

```bash
# Start all services
./start-all.sh

# Stop all services
./stop-all.sh

# Check service status
./status.sh
```

## 📁 Directory Structure

```
docker/
├── doc/                    # Documentation service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
├── web/                    # Frontend service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
├── api/                    # API service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   ├── start_api.py
│   └── README.md
├── wiki/                   # Wiki service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   ├── requirements.txt
│   └── README.md
├── start-all.sh           # Start all services
├── stop-all.sh            # Stop all services
├── status.sh              # Check status
└── README.md              # This file
```

## 🌐 Service Access

After successful startup, you can access each service at the following addresses:

- **Documentation**: http://localhost:8000
- **Frontend**: http://localhost:5177
- **API**: http://localhost:18200
- **Wiki**: http://localhost:21923/mcp

## 🔧 Development Mode

For development, you can mount source code to enable hot reload:

```bash
# API service development mode
cd docker/api
docker-compose -f docker-compose.dev.yml up -d

# Frontend service development mode
cd docker/web
docker-compose -f docker-compose.dev.yml up -d
```

## 📊 Monitoring

Each service includes health checks and log output:

```bash
# View service logs
docker-compose logs -f

# Check health status
docker-compose ps
```

## 🔒 Production Deployment

Production environment recommended configurations:

1. **Environment Variables**: Use `.env` files to manage sensitive configurations
2. **Network Isolation**: Configure firewall rules
3. **Log Collection**: Centralized log collection and management
4. **Monitoring Alerts**: Configure Prometheus + Grafana

## ⚠️ Important Notes

1. Ensure ports 8000, 5177, 18200, 21923 are not occupied
2. First-time build may require significant time to download dependencies
3. Recommend using Docker Compose v2.0+
4. Set appropriate resource limits for production environment