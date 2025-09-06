"""
API 服务器模块
负责处理 MCPStore 的 API 服务器启动功能
"""

import logging

logger = logging.getLogger(__name__)


class APIServerMixin:
    """API 服务器 Mixin"""
    
    def start_api_server(self,
                        host: str = "0.0.0.0",
                        port: int = 18200,
                        reload: bool = False,
                        log_level: str = "info",
                        auto_open_browser: bool = False,
                        show_startup_info: bool = True) -> None:
        """
        启动API服务器

        这个方法会启动一个HTTP API服务器，提供RESTful接口来访问当前MCPStore实例的功能。
        服务器会自动使用当前store的配置和数据空间。

        Args:
            host: 服务器监听地址，默认"0.0.0.0"（所有网络接口）
            port: 服务器监听端口，默认18200
            reload: 是否启用自动重载（开发模式），默认False
            log_level: 日志级别，可选值: "critical", "error", "warning", "info", "debug", "trace"
            auto_open_browser: 是否自动打开浏览器，默认False
            show_startup_info: 是否显示启动信息，默认True

        Note:
            - 此方法会阻塞当前线程直到服务器停止
            - 使用Ctrl+C可以优雅地停止服务器
            - 如果使用了数据空间，API会自动使用对应的工作空间
            - 本地服务的子进程会被正确管理和清理

        Example:
            # 基本使用
            store = MCPStore.setup_store("./my_workspace/mcp.json")
            store.start_api_server()

            # 开发模式
            store.start_api_server(reload=True, auto_open_browser=True)

            # 自定义配置
            store.start_api_server(host="localhost", port=8080, log_level="debug")
        """
        try:
            import uvicorn
            import webbrowser
            from pathlib import Path

            logger.info(f"Starting API server for store: data_space={self.is_using_data_space()}")

            if show_startup_info:
                print("🚀 Starting MCPStore API Server...")
                print(f"   Host: {host}:{port}")
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

            # 设置全局store实例供API使用（在启动服务器之前）
            self._setup_api_store_instance()
            logger.info(f"Global store instance set for API: {type(self).__name__}")

            # 自动打开浏览器
            if auto_open_browser:
                import threading
                import time

                def open_browser():
                    time.sleep(2)  # 等待服务器启动
                    try:
                        webbrowser.open(f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}")
                    except Exception as e:
                        if show_startup_info:
                            print(f"⚠️ Failed to open browser: {e}")

                threading.Thread(target=open_browser, daemon=True).start()

            # 启动API服务器
            # 不使用factory模式，直接创建app实例以保持全局变量
            from mcpstore.scripts.api_app import create_app
            app = create_app()

            uvicorn.run(
                app,
                host=host,
                port=port,
                reload=reload,
                log_level=log_level
            )

        except KeyboardInterrupt:
            if show_startup_info:
                print("\n🛑 Server stopped by user")
        except ImportError as e:
            raise RuntimeError(
                "Failed to import required dependencies for API server. "
                "Please install uvicorn: pip install uvicorn"
            ) from e
        except Exception as e:
            if show_startup_info:
                print(f"❌ Failed to start server: {e}")
            raise

    def _setup_api_store_instance(self):
        """设置API使用的store实例"""
        # 将当前store实例设置为全局实例，供API使用
        import mcpstore.scripts.api_app as api_app
        import mcpstore.scripts.api_dependencies as api_deps
        
        # 设置api_app的全局实例
        api_app._global_store_instance = self
        
        # 设置api_dependencies的全局实例
        api_deps.set_global_store(self)
        
        logger.info(f"Set global store instance: data_space={self.is_using_data_space()}, workspace={self.get_workspace_dir()}")
        logger.info(f"Global instance id: {id(self)}, api module instance id: {id(api_app._global_store_instance)}")
