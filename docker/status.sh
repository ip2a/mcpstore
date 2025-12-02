#!/bin/bash

# MCPStore Service Status Check Script

echo "📊 MCPStore Service Status"
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
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if port is open
check_port() {
    local port=$1
    if lsof -i :$port >/dev/null 2>&1; then
        return 0
    fi
    return 1
}

# Check Docker Container Status
check_container() {
    local service="mcpstore-$service"
    if docker ps --format "table {{.Names}}\t{{.Status}}" | grep -q "$service"; then
        return 0
    fi
    return 1
}

# Check HTTP response
check_http() {
    local url=$1
    if curl -s -o /dev/null -w "%{http_code}" "$url" | grep -q "200\|404"; then
        return 0
    fi
    return 1
}

# Output service status
print_service_status() {
    local service=$1
    local port=${ports[$service]}
    local container_name="mcpstore-$service"
    local url="http://localhost:$port"

    printf "%-10s (Port %-5s): " "$service" "$port"

    # Check port
    if check_port $port; then
        echo -ne "${GREEN}✓ Port Open${NC}"
    else
        echo -ne "${RED}✗ Port Closed${NC}"
        return
    fi

    # Check container
    if docker ps --format "table {{.Names}}" | grep -q "$container_name"; then
        echo -ne " | ${GREEN}✓ Container Running${NC}"
    else
        echo -ne " | ${RED}✗ Container Stopped${NC}"
    fi

    # Check HTTP response
    case $service in
        "api")
            if curl -s -o /dev/null -w "%{http_code}" "$url/health" | grep -q "200"; then
                echo -ne " | ${GREEN}✓ API Health${NC}"
            else
                echo -ne " | ${YELLOW}⚠ API Error${NC}"
            fi
            ;;
        "wiki")
            if curl -s -o /dev/null -w "%{http_code}" "$url/mcp" | grep -q "200\|404"; then
                echo -ne " | ${GREEN}✓ Wiki Health${NC}"
            else
                echo -ne " | ${YELLOW}⚠ Wiki Error${NC}"
            fi
            ;;
        *)
            if check_http "$url"; then
                echo -ne " | ${GREEN}✓ HTTP Response${NC}"
            else
                echo -ne " | ${YELLOW}⚠ HTTP Error${NC}"
            fi
            ;;
    esac

    echo ""
}

# Display container details
echo -e "${BLUE}Docker Container Status:${NC}"
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" | grep mcpstore || echo "  No running MCPStore containers"

echo ""
echo -e "${BLUE}Service Health Status:${NC}"
for service in "${services[@]}"; do
    print_service_status "$service"
done

echo ""
echo -e "${BLUE}Service Access URLs:${NC}"
for service in "${services[@]}"; do
    local port=${ports[$service]}
    echo "  - $service: http://localhost:$port"
done

echo ""
echo -e "${BLUE}Log View Commands:${NC}"
for service in "${services[@]}"; do
    echo "  - $service: cd docker/$service && docker-compose logs -f"
done

echo ""
echo "🔄 Restart Services: ./start-all.sh"
echo "🛑 Stop All Services: ./stop-all.sh"