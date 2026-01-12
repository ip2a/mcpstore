"""
API 服务器模块
负责处理 MCPStore 的 API 服务器启动功能
"""

import logging

logger = logging.getLogger(__name__)


class APIServerMixin:
    """API 服务器 Mixin"""
    
    def start_api_server(
        self,
        host: str = "0.0.0.0",
        port: int = 18200,
        reload: bool = False,
        log_level: str = "info",
        auto_open_browser: bool = False,
        show_startup_info: bool = True,
        url_prefix: str = ""  # New: URL prefix parameter
    ) -> None:
        """
        启动 API 服务器（改进版）

        这个方法会启动一个 HTTP API 服务器，提供 RESTful 接口来访问当前 MCPStore 实例的功能。
        服务器会自动使用当前 store 的配置和数据空间。

        Args:
            host: 服务器监听地址，默认 "0.0.0.0"（所有网络接口）
            port: 服务器监听端口，默认 18200
            reload: 是否启用自动重载（开发模式），默认 False
            log_level: 日志级别，可选值: "critical", "error", "degraded", "info", "debug", "trace"
            auto_open_browser: 是否自动打开浏览器，默认 False
            show_startup_info: 是否显示启动信息，默认 True
            url_prefix: URL 前缀，如 "/api/v1"。默认为空（无前缀）

        Note:
            - 此方法会阻塞当前线程直到服务器停止
            - 使用 Ctrl+C 可以优雅地停止服务器
            - 如果使用了数据空间，API 会自动使用对应的工作空间
            - 本地服务的子进程会被正确管理和清理

        Example:
            # 基本使用（无前缀）
            store = MCPStore.setup_store()
            store.start_api_server()
            # 访问: http://localhost:18200/for_store/list_services

            # 使用 URL 前缀
            store.start_api_server(url_prefix="/api/v1")
            # 访问: http://localhost:18200/api/v1/for_store/list_services

            # 开发模式
            store.start_api_server(reload=True, auto_open_browser=True)

            # 自定义配置
            store.start_api_server(
                host="localhost",
                port=8080,
                log_level="debug",
                url_prefix="/api"
            )
        """
        try:
            import uvicorn
            import webbrowser
            from pathlib import Path

            logger.info(f"Starting API server for store: data_space={self.is_using_data_space()}")

            if show_startup_info:
                print("[START] Starting MCPStore API Server...")
                print(f"   Host: {host}:{port}")

                if url_prefix:
                    print(f"   URL Prefix: {url_prefix}")
                    base_url = f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}"
                    print(f"   Example: {base_url}{url_prefix}/for_store/list_services")
                else:
                    base_url = f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}"
                    print(f"   Example: {base_url}/for_store/list_services")

                if self.is_using_data_space():
                    workspace_dir = self.get_workspace_dir()
                    print(f"   Data Space: {workspace_dir}")
                    print(f"   MCP Config: {self.config.json_path}")
                else:
                    print(f"   MCP Config: {self.config.json_path}")

                if reload:
                    print("   Mode: Development (auto-reload enabled)")
                else:
                    print("   Mode: Production")

                print("   Press Ctrl+C to stop")
                print()

            # 自动打开浏览器
            if auto_open_browser:
                import threading
                import time

                def open_browser():
                    time.sleep(2)  # 等待服务器启动
                    try:
                        base_url = f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}"
                        doc_url = f"{base_url}{url_prefix}/docs" if url_prefix else f"{base_url}/docs"
                        webbrowser.open(doc_url)
                    except Exception as e:
                        if show_startup_info:
                            print(f"[DEGRADED] Failed to open browser: {e}")

                threading.Thread(target=open_browser, daemon=True).start()

            # Create app instance and pass current store and URL prefix
            # Note: 延迟导入避免 core 层在模块加载时就依赖 scripts 层
            from mcpstore.scripts.api_app import create_app
            app = create_app(store=self, url_prefix=url_prefix)

            # 启动 API 服务器
            uvicorn.run(
                app,
                host=host,
                port=port,
                reload=reload,
                log_level=log_level
            )

        except KeyboardInterrupt:
            if show_startup_info:
                print("\n[STOPPED] Server stopped by user")
        except ImportError as e:
            raise RuntimeError(
                "Failed to import required dependencies for API server. "
                "Please install uvicorn: pip install uvicorn"
            ) from e
        except Exception as e:
            if show_startup_info:
                print(f" Failed to start server: {e}")
            raise
