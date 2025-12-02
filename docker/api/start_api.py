#!/usr/bin/env python3
"""
MCPStore API Service Startup Script
Based on docker/exp_run_service.py
"""

import sys
import os

# Add source path to Python path
sys.path.insert(0, '/app/src')

from mcpstore import MCPStore


def main():
    """Start MCPStore API server"""
    print("=" * 60)
    print("MCPStore API Server Starting")
    print("=" * 60)
    print(f"[i] Host: 0.0.0.0")
    print(f"[i] Port: 18200")
    print(f"[i] Debug: {os.getenv('DEBUG', 'False').lower() == 'true'}")
    print(f"[i] Log Level: {os.getenv('LOG_LEVEL', 'info')}")
    print("-" * 60)

    # Initialize production store
    print("[i] Initializing MCPStore...")

    # Configure using environment variables
    debug = os.getenv('DEBUG', 'False').lower() == 'true'
    log_level = os.getenv('LOG_LEVEL', 'info')

    try:
        store = MCPStore.setup_store(debug=debug)
        print("[✓] MCPStore initialized successfully")

        # Start API server
        print("[i] Starting API server...")

        store.start_api_server(
            host='0.0.0.0',
            port=18200,
            log_level=log_level,
            reload=False,  # Do not enable hot reload in production
            auto_open_browser=False,
            show_startup_info=True
        )

    except Exception as e:
        print(f"[!] Startup failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()