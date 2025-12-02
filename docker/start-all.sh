#!/bin/bash

# MCPStore All Services Startup Script

echo "🚀 Starting MCPStore All Services..."
echo "================================"

# Define service list
services=("doc" "web" "api" "wiki")

# Define port mapping
declare -A ports=(
    ["doc"]="8000"
    ["web"]="5177"
    ["api"]="18200"
    ["wiki"]="21923"
)

# Color definitions
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if port is occupied
check_port() {
    local port=$1
    if lsof -i :$port >/dev/null 2>&1; then
        echo -e "${RED}Warning: Port $port is already in use${NC}"
        return 1
    fi
    return 0
}

# Start single service
start_service() {
    local service=$1
    local port=${ports[$service]}

    echo -e "${YELLOW}Starting $service service (port: $port)...${NC}"

    if check_port $port; then
        cd "$(dirname "$0")/$service"
        if docker-compose up -d --build; then
            echo -e "${GREEN}✓ $service service started successfully${NC}"
            cd - > /dev/null
            return 0
        else
            echo -e "${RED}✗ $service service startup failed${NC}"
            cd - > /dev/null
            return 1
        fi
    else
        echo -e "${RED}Skipping $service service (port occupied)${NC}"
        return 1
    fi
}

# Check Docker and Docker Compose
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker is not installed${NC}"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}Error: Docker Compose is not installed${NC}"
    exit 1
fi

# Start all services
failed_services=()
success_count=0

for service in "${services[@]}"; do
    if start_service "$service"; then
        ((success_count++))
    else
        failed_services+=("$service")
    fi
    echo ""
done

# Output startup results
echo "================================"
echo -e "${GREEN}Successfully started $success_count/${#services[@]} services${NC}"

if [ ${#failed_services[@]} -gt 0 ]; then
    echo -e "${RED}Failed services: ${failed_services[*]}${NC}"
fi

echo ""
echo "🌐 Service Access URLs:"
for service in "${services[@]}"; do
    local port=${ports[$service]}
    echo "  - $service: http://localhost:$port"
done

echo ""
echo "📊 Check service status: ./status.sh"
echo "🛑 Stop all services: ./stop-all.sh"