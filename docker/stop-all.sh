#!/bin/bash

# MCPStore All Services Stop Script

echo "🛑 Stopping MCPStore All Services..."
echo "================================"

# Define service list
services=("doc" "web" "api" "wiki")

# Color definitions
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Stop single service
stop_service() {
    local service=$1
    echo -e "${YELLOW}Stopping $service service...${NC}"

    cd "$(dirname "$0")/$service"
    if docker-compose down; then
        echo -e "${GREEN}✓ $service service stopped${NC}"
        cd - > /dev/null
        return 0
    else
        echo -e "${RED}✗ $service service stop failed${NC}"
        cd - > /dev/null
        return 1
    fi
}

# Stop all services
failed_services=()
success_count=0

for service in "${services[@]}"; do
    if stop_service "$service"; then
        ((success_count++))
    else
        failed_services+=("$service")
    fi
    echo ""
done

# Clean up unused containers and networks (optional)
read -p "Clean up unused Docker resources? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Cleaning up unused Docker resources...${NC}"
    docker system prune -f
fi

# Output stop results
echo "================================"
echo -e "${GREEN}Successfully stopped $success_count/${#services[@]} services${NC}"

if [ ${#failed_services[@]} -gt 0 ]; then
    echo -e "${RED}Failed services: ${failed_services[*]}${NC}"
fi

echo ""
echo "🔄 Restart all services: ./start-all.sh"
echo "📊 Check service status: ./status.sh"