"""
MCPOrchestrator Health Monitoring Module
Health monitoring module - contains detailed health checks and status management
"""

import asyncio
import logging
import time
from typing import Dict, List, Any, Optional, Tuple

from fastmcp import Client
from mcpstore.core.lifecycle import HealthStatus, HealthCheckResult
from mcpstore.core.configuration.config_processor import ConfigProcessor

logger = logging.getLogger(__name__)

class HealthMonitoringMixin:
    """Health monitoring mixin class"""

    async def check_service_health_detailed(self, name: str, client_id: Optional[str] = None) -> HealthCheckResult:
        """
        Detailed service health check, returns complete health status information

        Args:
            name: Service name
            client_id: Optional client ID for multi-client environments

        Returns:
            HealthCheckResult: Detailed health check results
        """
        start_time = time.time()
        try:
            # Get service configuration
            service_config, fastmcp_config = await self._get_service_config_for_health_check(name, client_id)
            if not service_config:
                error_msg = f"Service configuration not found for {name}"
                logger.debug(error_msg)
                return self.health_manager.record_health_check(
                    name, 0.0, False, error_msg, service_config
                )

            # Quick network connectivity check (HTTP services only)
            if service_config.get("url"):
                if not await self._quick_network_check(service_config["url"]):
                    error_msg = f"Quick network check failed for {name}"
                    logger.debug(error_msg)
                    response_time = time.time() - start_time
                    return self.health_manager.record_health_check(
                        name, response_time, False, error_msg, service_config
                    )

            # 获取智能调整的超时时间
            timeout_seconds = self.health_manager.get_service_timeout(name, service_config)
            logger.debug(f"Using timeout {timeout_seconds}s for service {name}")

            # 创建新的客户端实例
            client = Client(fastmcp_config)

            try:
                async with asyncio.timeout(timeout_seconds):
                    async with client:
                        await client.ping()
                        # 成功响应，记录响应时间
                        response_time = time.time() - start_time
                        return self.health_manager.record_health_check(
                            name, response_time, True, None, service_config
                        )
            except asyncio.TimeoutError:
                response_time = time.time() - start_time
                error_msg = f"Health check timeout after {timeout_seconds}s"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except ConnectionError as e:
                response_time = time.time() - start_time
                error_msg = f"Connection error: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except FileNotFoundError as e:
                response_time = time.time() - start_time
                error_msg = f"Command service file not found: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except PermissionError as e:
                response_time = time.time() - start_time
                error_msg = f"Permission error: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except Exception as e:
                response_time = time.time() - start_time
                # 使用ConfigProcessor提供更友好的错误信息
                friendly_error = ConfigProcessor.get_user_friendly_error(str(e))

                # 检查是否是文件系统相关错误
                if self._is_filesystem_error(e):
                    logger.debug(f"Filesystem error for {name} (client_id={client_id}): {friendly_error}")
                # 检查是否是网络相关错误
                elif self._is_network_error(e):
                    logger.debug(f"Network error for {name} (client_id={client_id}): {friendly_error}")
                elif "validation errors" in str(e).lower():
                    # 配置验证错误通常是由于用户自定义字段，这是正常的
                    logger.debug(f"Configuration has user-defined fields for {name} (client_id={client_id}): {friendly_error}")
                    # 对于配置验证错误，我们认为服务是"可用但需要配置清理"的状态
                    logger.info(f"Service {name} has configuration validation issues but may still be functional")
                else:
                    logger.debug(f"Health check failed for {name} (client_id={client_id}): {friendly_error}")

                return self.health_manager.record_health_check(
                    name, response_time, False, friendly_error, service_config
                )
            finally:
                # 确保客户端被正确关闭
                try:
                    await client.close()
                except Exception:
                    pass  # 忽略关闭时的错误

        except Exception as e:
            response_time = time.time() - start_time
            error_msg = f"Health check failed: {str(e)}"
            logger.debug(f"{error_msg} for {name} (client_id={client_id})")
            return self.health_manager.record_health_check(
                name, response_time, False, error_msg, {}
            )

    def get_service_comprehensive_status(self, service_name: str, client_id: str = None) -> str:
        """获取服务的完整状态（包括重连状态）"""
        try:
            agent_key = client_id or self.client_manager.global_agent_store_id
            
            # 从生命周期管理器获取状态
            if hasattr(self, 'lifecycle_manager') and self.lifecycle_manager:
                lifecycle_state = self.lifecycle_manager.get_service_state(agent_key, service_name)
                if lifecycle_state:
                    return lifecycle_state.value
            
            # 从注册表获取基本状态
            if self.registry.has_service(agent_key, service_name):
                return "connected"
            else:
                return "disconnected"
                
        except Exception as e:
            logger.error(f"Error getting comprehensive status for {service_name}: {e}")
            return "unknown"

    async def _get_service_config_for_health_check(self, name: str, client_id: Optional[str] = None) -> Tuple[Optional[Dict[str, Any]], Optional[Dict[str, Any]]]:
        """获取用于健康检查的服务配置"""
        try:
            # 优先使用已处理的client配置，如果没有则使用原始配置
            if client_id:
                client_config = self.client_manager.get_client_config(client_id)
                if client_config and name in client_config.get("mcpServers", {}):
                    # 使用已处理的client配置
                    service_config = client_config["mcpServers"][name]
                    fastmcp_config = client_config
                    logger.debug(f"Using processed client config for health check: {name}")
                    return service_config, fastmcp_config
                else:
                    # 回退到原始配置
                    service_config = self.mcp_config.get_service_config(name)
                    if not service_config:
                        return None, None

                    # 使用ConfigProcessor处理配置
                    user_config = {"mcpServers": {name: service_config}}
                    fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                    logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                    # 检查ConfigProcessor是否移除了服务（配置错误）
                    if name not in fastmcp_config.get("mcpServers", {}):
                        logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                        return None, None

                    return service_config, fastmcp_config
            else:
                # 没有client_id，使用原始配置
                service_config = self.mcp_config.get_service_config(name)
                if not service_config:
                    return None, None

                # 使用ConfigProcessor处理配置
                user_config = {"mcpServers": {name: service_config}}
                fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                # 检查ConfigProcessor是否移除了服务（配置错误）
                if name not in fastmcp_config.get("mcpServers", {}):
                    logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                    return None, None

                return service_config, fastmcp_config
        except Exception as e:
            logger.error(f"Error getting service config for health check {name}: {e}")
            return None, None

    async def _quick_network_check(self, url: str) -> bool:
        """快速网络连通性检查"""
        try:
            from urllib.parse import urlparse
            import asyncio

            parsed = urlparse(url)
            if not parsed.hostname:
                return True  # 无法解析主机名，跳过检查

            # 🔧 修复：对MCP端点使用TCP连接检查而不是HTTP GET请求
            # MCP服务器期望POST请求，GET请求会返回400错误
            try:
                reader, writer = await asyncio.wait_for(
                    asyncio.open_connection(parsed.hostname, parsed.port or (443 if parsed.scheme == 'https' else 80)),
                    timeout=2.0  # 2秒超时
                )
                writer.close()
                await writer.wait_closed()
                return True
            except Exception:
                return False

        except Exception:
            return False  # 任何异常都认为网络不通

    def _normalize_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """规范化服务配置，确保包含必要的字段"""
        normalized = service_config.copy()
        
        # 确保有基本字段
        if "name" not in normalized and "url" in normalized:
            # 从URL推断名称
            url = normalized["url"]
            if url.startswith("http"):
                # HTTP服务
                normalized["name"] = url.split("/")[-1] or "http_service"
            else:
                normalized["name"] = "unknown_service"
        
        # 确保有传输类型
        if "transport" not in normalized:
            if "command" in normalized:
                normalized["transport"] = "stdio"
            elif "url" in normalized:
                normalized["transport"] = "http"
        
        return normalized
