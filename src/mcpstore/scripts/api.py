"""
MCPStore API main route registration file
Integrates routes from all sub-modules, providing a unified API entry point

Refactoring notes:
- The original 2391-line api.py file has been split by functional modules into:
  * api_models.py - All response models
  * api_decorators.py - Decorators and utility functions
  * api_store.py - Store-level routes
  * api_agent.py - Agent-level routes
  * api_monitoring.py - Monitoring-related routes
- This file is responsible for unified registration of all sub-routes, maintaining API interface compatibility
"""

from fastapi import APIRouter

from .api_agent import agent_router
from .api_monitoring import monitoring_router
# Import all sub-route modules
from .api_store import store_router

# Import dependency injection functions (maintain compatibility)

# Create main router
router = APIRouter()

# Register all sub-routes
# Store-level operation routes
router.include_router(store_router, tags=["Store Operations"])

# Agent-level operation routes
router.include_router(agent_router, tags=["Agent Operations"])

# Monitoring and statistics routes
router.include_router(monitoring_router, tags=["Monitoring & Statistics"])

# Maintain backward compatibility - export commonly used functions and classes
# This way existing import statements can still work normally

# Route statistics information (for debugging)
def get_route_info():
    """Get route statistics information"""
    total_routes = len(router.routes)
    store_routes = len(store_router.routes)
    agent_routes = len(agent_router.routes)
    monitoring_routes = len(monitoring_router.routes)

    return {
        "total_routes": total_routes,
        "store_routes": store_routes,
        "agent_routes": agent_routes,
        "monitoring_routes": monitoring_routes,
        "modules": {
            "api_store.py": f"{store_routes} routes",
            "api_agent.py": f"{agent_routes} routes",
            "api_monitoring.py": f"{monitoring_routes} routes"
        }
    }

# Health check endpoint (simple root path check)
@router.get("/", tags=["System"])
async def api_root():
    """API root path - system information"""
    from mcpstore.core.models.common import APIResponse

    route_info = get_route_info()

    return APIResponse(
        success=True,
        data={
            "message": "MCPStore API Server",
            "version": "0.6.0",
            "status": "running",
            "routes": route_info,
            "documentation": "/docs",
            "openapi": "/openapi.json"
        },
        message="MCPStore API is running successfully"
    )
