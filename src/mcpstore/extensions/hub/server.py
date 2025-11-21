"""
Hub Server Module
Hub服务器模块 - 生成基于FastMCP的Hub服务器代码
"""

import asyncio
import json
import logging
import subprocess
import sys
import tempfile
from typing import List, Tuple

from .types import HubConfig, HubServiceInfo

logger = logging.getLogger(__name__)


class HubServerGenerator:
    """
    Hub服务器生成器
    
    负责动态生成基于FastMCP的Hub服务器代码和配置文件。
    生成的服务器将作为独立进程运行，代理多个上游MCP服务。
    
    特点：
    - 基于FastMCP框架，自动处理MCP协议
    - 动态生成代理工具，无需预编译
    - 支持基础路由功能
    - 完整的错误处理和日志记录
    """
    
    def __init__(self):
        """初始化Hub服务器生成器"""
        self._script_template = self._get_server_script_template()
        logger.debug("HubServerGenerator initialized")
    
    async def generate_server_files_async(
        self, 
        package_name: str,
        services: List[HubServiceInfo], 
        config: HubConfig,
        port: int
    ) -> Tuple[str, str]:
        """
        异步生成Hub服务器文件
        
        Args:
            package_name: Hub包名
            services: 服务列表
            config: Hub配置
            port: 服务端口
            
        Returns:
            Tuple[str, str]: (脚本文件路径, 配置文件路径)
        """
        try:
            logger.info(f"Generating server files for Hub '{package_name}'")
            
            # 1. 生成服务器脚本
            script_content = self._generate_server_script(package_name, services, config, port)
            script_file = await self._write_temp_file(script_content, suffix='.py')
            
            # 2. 生成配置文件
            config_content = self._generate_config_file(package_name, services, config, port)
            config_file = await self._write_temp_file(config_content, suffix='.json')
            
            logger.info(f"Generated server files: script={script_file}, config={config_file}")
            return script_file, config_file
            
        except Exception as e:
            logger.error(f"Failed to generate server files for '{package_name}': {e}")
            raise
    
    def generate_server_files(
        self, 
        package_name: str,
        services: List[HubServiceInfo], 
        config: HubConfig,
        port: int
    ) -> Tuple[str, str]:
        """
        同步生成Hub服务器文件
        
        Args:
            package_name: Hub包名
            services: 服务列表
            config: Hub配置
            port: 服务端口
            
        Returns:
            Tuple[str, str]: (脚本文件路径, 配置文件路径)
        """
        return asyncio.run(self.generate_server_files_async(package_name, services, config, port))
    
    async def start_subprocess_async(self, script_file: str, config_file: str, port: int) -> subprocess.Popen:
        """
        异步启动Hub服务器子进程
        
        Args:
            script_file: 脚本文件路径
            config_file: 配置文件路径
            port: 服务端口
            
        Returns:
            subprocess.Popen: 子进程对象
        """
        try:
            cmd = [
                sys.executable,  # Python解释器路径
                script_file,     # Hub服务器脚本
                "--config", config_file,
                "--port", str(port)
            ]
            
            logger.info(f"Starting Hub subprocess: {' '.join(cmd)}")
            
            # 启动子进程
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                universal_newlines=True,
                # 在Windows上避免创建新的控制台窗口
                creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
            )
            
            logger.info(f"Hub subprocess started with PID: {process.pid}")
            return process
            
        except Exception as e:
            logger.error(f"Failed to start Hub subprocess: {e}")
            raise
    
    def start_subprocess(self, script_file: str, config_file: str, port: int) -> subprocess.Popen:
        """
        同步启动Hub服务器子进程
        
        Args:
            script_file: 脚本文件路径
            config_file: 配置文件路径
            port: 服务端口
            
        Returns:
            subprocess.Popen: 子进程对象
        """
        return asyncio.run(self.start_subprocess_async(script_file, config_file, port))
    
    def _generate_server_script(
        self, 
        package_name: str,
        services: List[HubServiceInfo], 
        config: HubConfig,
        port: int
    ) -> str:
        """
        生成Hub服务器Python脚本
        
        Args:
            package_name: Hub包名
            services: 服务列表
            config: Hub配置
            port: 服务端口
            
        Returns:
            str: 生成的Python脚本内容
        """
        # 生成工具代理代码
        proxy_tools_code = self._generate_proxy_tools_code(services, port)
        
        # 生成认证配置代码
        auth_imports, auth_setup = self._generate_auth_config_code(config)
        
        # 替换模板变量
        script_content = self._script_template.format(
            package_name=package_name,
            services_count=len(services),
            proxy_tools_code=proxy_tools_code,
            config_description=config.description or f"Hub服务器包含{len(services)}个上游MCP服务",
            auth_imports=auth_imports,
            auth_setup=auth_setup
        )
        
        return script_content
    
    def _generate_proxy_tools_code(self, services: List[HubServiceInfo], port: int = 3000) -> str:
        """
        生成代理工具的Python代码
        
        Args:
            services: 服务列表
            
        Returns:
            str: 代理工具的Python代码
        """
        tools_code_lines = []
        
        # 为每个服务的每个工具生成代理函数
        for service in services:
            for tool in service.tools:
                tool_name = tool.get("name", "unknown")
                tool_description = tool.get("description", f"Tool from {service.name}")
                
                # 生成合法的Python函数名（替换非法字符）
                safe_service_name = service.name.replace('-', '_').replace('.', '_')
                safe_tool_name = tool_name.replace('-', '_').replace('.', '_')
                
                # 生成代理工具代码
                tool_code = f'''
        @self.mcp.tool(
            name="{service.name}_{tool_name}",
            description="[{service.name}] {tool_description}"
        )
        async def proxy_{safe_service_name}_{safe_tool_name}() -> dict:
            """代理工具: {service.name}.{tool_name}"""
            # TODO: 实际调用上游服务的工具
            # 当前返回模拟结果，不使用**kwargs以兼容FastMCP
            return {{
                "proxy_result": "success",
                "service": "{service.name}",
                "tool": "{tool_name}",
                "note": "这是Hub代理调用的模拟结果",
                "upstream_service": {{
                    "name": "{service.name}",
                    "transport_type": "{service.transport_type}",
                    "status": "{service.status}"
                }}
            }}'''
                
                tools_code_lines.append(tool_code)
        
        # 添加Hub信息工具
        # 生成服务信息的静态数据
        services_data = []
        for s in services:
            services_data.append({
                "name": s.name,
                "transport_type": s.transport_type,
                "status": s.status,
                "tools_count": len(s.tools)
            })
        
        hub_name = f"{services[0].name}-hub" if services else "empty-hub"
        
        info_tool_code = f'''
        @self.mcp.tool(
            name="hub_info",
            description="获取Hub服务器信息和状态"
        )
        async def get_hub_info() -> dict:
            """获取Hub服务器信息"""
            return {{
                "hub_name": "{hub_name}",
                "service_count": {len(services)},
                "services": {services_data},
                "routing": {{
                    "basic_routing": True,
                    "endpoints": ["http://localhost:{port}/mcp"]
                }}
            }}'''
        
        tools_code_lines.append(info_tool_code)
        
        return '\n'.join(tools_code_lines)
    
    def _generate_auth_config_code(self, config: HubConfig) -> tuple[str, str]:
        """
        生成认证配置代码
        
        Args:
            config: Hub配置
            
        Returns:
            tuple[str, str]: (认证导入代码, 认证设置代码)
        """
        if not config.auth_enabled or not config.fastmcp_auth:
            return "", "# 无认证配置\n        self.mcp = FastMCP(name=package_name)"
        
        auth_config = config.fastmcp_auth
        auth_type = auth_config.get("type", "BearerAuthProvider")
        
        # 生成导入代码
        if auth_type == "BearerAuthProvider":
            auth_imports = """
from fastmcp.server.auth import BearerAuthProvider
from fastmcp.server.dependencies import get_access_token, AccessToken"""
            
            auth_setup = f'''
        # 设置Bearer Token认证
        auth_provider = BearerAuthProvider(
            jwks_uri="{auth_config.get('jwks_uri', '')}",
            issuer="{auth_config.get('issuer', '')}",
            audience="{auth_config.get('audience', '')}",
            algorithm="{auth_config.get('algorithm', 'RS256')}"
        )
        
        # 创建带认证的FastMCP实例
        self.mcp = FastMCP(name=package_name, auth=auth_provider)'''
        
        elif auth_type == "GoogleProvider":
            auth_imports = """
from fastmcp.server.auth.providers.google import GoogleProvider
from fastmcp.server.dependencies import get_access_token, AccessToken"""
            
            auth_setup = f'''
        # 设置Google OAuth认证
        auth_provider = GoogleProvider(
            client_id="{auth_config.get('client_id', '')}",
            client_secret="{auth_config.get('client_secret', '')}",
            base_url="{auth_config.get('base_url', '')}",
            required_scopes={auth_config.get('required_scopes', ["openid", "email", "profile"])}
        )
        
        # 创建带认证的FastMCP实例
        self.mcp = FastMCP(name=package_name, auth=auth_provider)'''
        
        elif auth_type == "GitHubProvider":
            auth_imports = """
from fastmcp.server.auth.providers.github import GitHubProvider
from fastmcp.server.dependencies import get_access_token, AccessToken"""
            
            auth_setup = f'''
        # 设置GitHub OAuth认证
        auth_provider = GitHubProvider(
            client_id="{auth_config.get('client_id', '')}",
            client_secret="{auth_config.get('client_secret', '')}",
            base_url="{auth_config.get('base_url', '')}",
            required_scopes={auth_config.get('required_scopes', ["user"])}
        )
        
        # 创建带认证的FastMCP实例
        self.mcp = FastMCP(name=package_name, auth=auth_provider)'''
        
        elif auth_type == "AuthKitProvider":
            auth_imports = """
from fastmcp.server.auth.providers.workos import AuthKitProvider
from fastmcp.server.dependencies import get_access_token, AccessToken"""
            
            auth_setup = f'''
        # 设置WorkOS AuthKit认证
        auth_provider = AuthKitProvider(
            authkit_domain="{auth_config.get('authkit_domain', '')}",
            base_url="{auth_config.get('base_url', '')}"
        )
        
        # 创建带认证的FastMCP实例
        self.mcp = FastMCP(name=package_name, auth=auth_provider)'''
        
        else:
            # 默认无认证
            auth_imports = ""
            auth_setup = "# 无认证配置\n        self.mcp = FastMCP(name=package_name)"
        
        return auth_imports, auth_setup
    
    def _generate_config_file(
        self, 
        package_name: str,
        services: List[HubServiceInfo], 
        config: HubConfig,
        port: int
    ) -> str:
        """
        生成Hub配置文件
        
        Args:
            package_name: Hub包名
            services: 服务列表
            config: Hub配置
            port: 服务端口
            
        Returns:
            str: JSON格式的配置文件内容
        """
        config_data = {
            "hub": {
                "package_name": package_name,
                "description": config.description,
                "context_type": config.context_type,
                "target_id": config.target_id,
                "port": port,
                "basic_routing": config.basic_routing
            },
            "services": [
                {
                    "name": service.name,
                    "url": service.url,
                    "command": service.command,
                    "args": service.args,
                    "transport_type": service.transport_type,
                    "status": service.status,
                    "tools": service.tools
                }
                for service in services
            ],
            "routing": {
                "basic": {
                    "enabled": config.basic_routing,
                    "path": "/mcp",
                    "description": "Access all aggregated tools"
                }
            }
        }
        
        return json.dumps(config_data, indent=2, ensure_ascii=False)
    
    async def _write_temp_file(self, content: str, suffix: str = '.tmp') -> str:
        """
        写入临时文件
        
        Args:
            content: 文件内容
            suffix: 文件后缀
            
        Returns:
            str: 临时文件路径
        """
        with tempfile.NamedTemporaryFile(
            mode='w', 
            suffix=suffix, 
            delete=False,
            encoding='utf-8'
        ) as f:
            f.write(content)
            return f.name
    
    def _get_server_script_template(self) -> str:
        """
        获取Hub服务器脚本模板
        
        Returns:
            str: 服务器脚本模板
        """
        return '''#!/usr/bin/env python3
"""
MCPStore Hub Server - 自动生成的Hub服务器
基于FastMCP实现，代理多个上游MCP服务

Hub: {package_name}
Services: {services_count}
Description: {config_description}
"""

import sys
import json
import asyncio
import argparse
import logging
from typing import Dict, Any

# FastMCP依赖检查
try:
    from fastmcp import FastMCP{auth_imports}
except ImportError:
    print("Error: FastMCP not installed. Run: pip install fastmcp")
    sys.exit(1)

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class MCPStoreHubServer:
    """MCPStore Hub服务器"""
    
    def __init__(self, config_file: str, port: int):
        self.config_file = config_file
        self.port = port
        self.config = {{}}
        self.mcp = None
        
    async def load_config(self):
        """加载Hub配置"""
        try:
            with open(self.config_file, 'r', encoding='utf-8') as f:
                self.config = json.load(f)
            logger.info(f"Loaded config from {{self.config_file}}")
        except Exception as e:
            logger.error(f"Failed to load config: {{e}}")
            raise
    
    def setup_fastmcp_server(self):
        """设置FastMCP服务器和代理工具"""
        hub_info = self.config.get('hub', {{}})
        package_name = hub_info.get('package_name', 'hub-server')
        description = hub_info.get('description', 'MCPStore Hub Server')
        
{auth_setup}
        
        logger.info(f"FastMCP server '{{package_name}}' created")
        
        # 设置代理工具
        self._setup_proxy_tools()
        
        logger.info(f"Proxy tools configured for {{len(self.config.get('services', []))}} services")
    
    def _setup_proxy_tools(self):
        """设置代理工具"""
{proxy_tools_code}
    
    async def start(self):
        """启动Hub服务器"""
        try:
            # 加载配置
            await self.load_config()
            
            # 设置FastMCP服务器
            self.setup_fastmcp_server()
            
            # 启动信息
            hub_info = self.config.get('hub', {{}})
            services = self.config.get('services', [])
            
            print(f"*** MCPStore Hub Server Starting...")
            print(f"*** Hub: {{hub_info.get('package_name', 'unknown')}}")
            print(f"*** Port: {{self.port}}")
            print(f"*** Services: {{len(services)}}")
            print(f"*** Endpoint: http://localhost:{{self.port}}/mcp")
            print(f"*** Service List: {{[s.get('name', 'unknown') for s in services]}}")
            print(f"*** Available Tools: hub_info + {{sum(len(s.get('tools', [])) for s in services)}} proxy tools")
            print()
            
            # 启动FastMCP HTTP服务器
            logger.info(f"Starting FastMCP HTTP server on port {{self.port}}")
            await self.mcp.run_async(
                transport="streamable-http",
                host="0.0.0.0",
                port=self.port
            )
            
        except Exception as e:
            logger.error(f"Hub server startup failed: {{e}}")
            raise


async def main():
    """主函数"""
    parser = argparse.ArgumentParser(description='MCPStore Hub Server')
    parser.add_argument('--config', required=True, help='配置文件路径')
    parser.add_argument('--port', type=int, required=True, help='服务端口')
    
    args = parser.parse_args()
    
    try:
        # 创建并启动Hub服务器
        hub = MCPStoreHubServer(args.config, args.port)
        await hub.start()
    except KeyboardInterrupt:
        logger.info("Hub server stopped by user")
    except Exception as e:
        logger.error(f"Hub server error: {{e}}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
'''
