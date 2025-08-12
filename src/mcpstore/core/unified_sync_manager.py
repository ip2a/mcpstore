"""
Unified MCP Configuration Synchronization Manager

Core design principles:
1. mcp.json is the single source of truth
2. All configuration changes go through mcp.json, automatically sync to global_agent_store
3. Agent operations only manage their own space + mcp.json, Store operations only manage mcp.json
4. Automatic sync mechanism handles mcp.json → global_agent_store synchronization

Data space support:
- File monitoring based on orchestrator.mcp_config.json_path
- Support independent synchronization for different data spaces
"""

import asyncio
import logging
import os
import time
from pathlib import Path
from typing import Dict, Set, Optional, Any
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

logger = logging.getLogger(__name__)


class MCPFileHandler(FileSystemEventHandler):
    """MCP configuration file change handler"""
    
    def __init__(self, sync_manager):
        self.sync_manager = sync_manager
        self.mcp_filename = os.path.basename(sync_manager.mcp_json_path)
        
    def on_modified(self, event):
        """File modification event handling"""
        if event.is_directory:
            return

        # Only monitor target mcp.json file
        if os.path.basename(event.src_path) == self.mcp_filename:
            logger.debug(f"MCP config file modified: {event.src_path}")
            # Safely execute async method in correct event loop
            try:
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # If event loop is running, use call_soon_threadsafe
                    loop.call_soon_threadsafe(
                        lambda: asyncio.create_task(self.sync_manager.on_file_changed())
                    )
                else:
                    # 如果事件循环未运行，直接创建任务
                    asyncio.create_task(self.sync_manager.on_file_changed())
            except RuntimeError:
                # 如果没有事件循环，记录警告
                logger.warning("No event loop available for file change notification")


class UnifiedMCPSyncManager:
    """统一的MCP配置同步管理器"""
    
    def __init__(self, orchestrator):
        """
        初始化同步管理器
        
        Args:
            orchestrator: MCPOrchestrator实例
        """
        self.orchestrator = orchestrator
        # 确保使用绝对路径
        import os
        self.mcp_json_path = os.path.abspath(orchestrator.mcp_config.json_path)
        self.file_observer = None
        self.sync_lock = asyncio.Lock()
        self.debounce_delay = 1.0  # 防抖延迟（秒）
        self.sync_task = None
        self.last_change_time = None
        self.is_running = False
        
        logger.info(f"UnifiedMCPSyncManager initialized for: {self.mcp_json_path}")
        
    async def start(self):
        """启动同步管理器"""
        if self.is_running:
            logger.warning("Sync manager is already running")
            return
            
        try:
            logger.info("Starting unified MCP sync manager...")
            
            # 启动文件监听
            await self._start_file_watcher()

            # 🔧 执行启动时同步（始终启用）
            logger.info("Executing initial sync from mcp.json")
            await self.sync_global_agent_store_from_mcp_json()

            self.is_running = True
            logger.info("Unified MCP sync manager started successfully")
            
        except Exception as e:
            logger.error(f"Failed to start sync manager: {e}")
            await self.stop()
            raise
            
    async def stop(self):
        """停止同步管理器"""
        if not self.is_running:
            return
            
        logger.info("Stopping unified MCP sync manager...")
        
        # 停止文件监听
        if self.file_observer:
            self.file_observer.stop()
            self.file_observer.join()
            self.file_observer = None
            
        # 取消待执行的同步任务
        if self.sync_task and not self.sync_task.done():
            self.sync_task.cancel()
            
        self.is_running = False
        logger.info("Unified MCP sync manager stopped")
        
    async def _start_file_watcher(self):
        """启动mcp.json文件监听"""
        try:
            # 确保mcp.json文件存在
            if not os.path.exists(self.mcp_json_path):
                logger.warning(f"MCP config file not found: {self.mcp_json_path}")
                # 创建空配置文件
                os.makedirs(os.path.dirname(self.mcp_json_path), exist_ok=True)
                with open(self.mcp_json_path, 'w', encoding='utf-8') as f:
                    import json
                    json.dump({"mcpServers": {}}, f, indent=2)
                logger.info(f"Created empty MCP config file: {self.mcp_json_path}")
            
            # 创建文件监听器
            self.file_observer = Observer()
            handler = MCPFileHandler(self)
            
            # 监听mcp.json所在目录
            watch_dir = os.path.dirname(self.mcp_json_path)
            self.file_observer.schedule(handler, watch_dir, recursive=False)
            self.file_observer.start()
            
            logger.info(f"File watcher started for directory: {watch_dir}")
            
        except Exception as e:
            logger.error(f"Failed to start file watcher: {e}")
            raise
            
    async def on_file_changed(self):
        """文件变化回调（带防抖）"""
        try:
            self.last_change_time = time.time()
            
            # 取消之前的同步任务
            if self.sync_task and not self.sync_task.done():
                self.sync_task.cancel()
                
            # 启动防抖同步
            self.sync_task = asyncio.create_task(self._debounced_sync())
            
        except Exception as e:
            logger.error(f"Error handling file change: {e}")
            
    async def _debounced_sync(self):
        """防抖同步"""
        try:
            await asyncio.sleep(self.debounce_delay)
            
            # 检查是否有新的变化
            if self.last_change_time and time.time() - self.last_change_time >= self.debounce_delay:
                logger.info("Triggering auto-sync due to mcp.json changes")
                await self.sync_main_client_from_mcp_json()
                
        except asyncio.CancelledError:
            logger.debug("Debounced sync cancelled")
        except Exception as e:
            logger.error(f"Error in debounced sync: {e}")
            
    async def sync_global_agent_store_from_mcp_json(self):
        """从mcp.json同步global_agent_store（核心方法）"""
        async with self.sync_lock:
            try:
                logger.info("Starting global_agent_store sync from mcp.json")

                # 读取最新配置
                config = self.orchestrator.mcp_config.load_config()
                services = config.get("mcpServers", {})

                logger.debug(f"Found {len(services)} services in mcp.json")

                # 执行同步
                results = await self._sync_global_agent_store_services(services)

                logger.info(f"Global agent store sync completed: {results}")
                return results

            except Exception as e:
                logger.error(f"Global agent store sync failed: {e}")
                raise
                
    async def _sync_global_agent_store_services(self, target_services: Dict[str, Any]) -> Dict[str, Any]:
        """同步global_agent_store的服务"""
        try:
            global_agent_store_id = self.orchestrator.client_manager.global_agent_store_id

            # 获取当前global_agent_store的服务
            current_services = self._get_current_global_agent_store_services()
            
            # 计算差异
            current_names = set(current_services.keys())
            target_names = set(target_services.keys())
            
            to_add = target_names - current_names
            to_remove = current_names - target_names
            to_update = target_names & current_names
            
            logger.debug(f"Sync plan: +{len(to_add)} -{len(to_remove)} ~{len(to_update)}")
            
            # 执行同步
            results = {
                "added": [],
                "removed": [],
                "updated": [],
                "failed": []
            }
            
            # 1. 移除不再需要的服务
            for service_name in to_remove:
                try:
                    success = await self._remove_service_from_global_agent_store(service_name)
                    if success:
                        results["removed"].append(service_name)
                        logger.debug(f"Removed service: {service_name}")
                    else:
                        results["failed"].append(f"remove:{service_name}")
                except Exception as e:
                    logger.error(f"Failed to remove service {service_name}: {e}")
                    results["failed"].append(f"remove:{service_name}:{e}")
            
            # 2. 添加/更新服务（新逻辑：操作缓存映射，然后异步持久化）
            services_to_register = {}
            for service_name in (to_add | to_update):
                try:
                    # 🔧 新逻辑：直接操作缓存映射而不是直接操作文件
                    success = await self._add_service_to_cache_mapping(
                        agent_id=global_agent_store_id,
                        service_name=service_name,
                        service_config=target_services[service_name]
                    )

                    if success:
                        services_to_register[service_name] = target_services[service_name]
                        if service_name in to_add:
                            results["added"].append(service_name)
                            logger.debug(f"Added service to cache: {service_name}")
                        else:
                            results["updated"].append(service_name)
                            logger.debug(f"Updated service in cache: {service_name}")
                    else:
                        action = "add" if service_name in to_add else "update"
                        results["failed"].append(f"{action}:{service_name}")

                except Exception as e:
                    action = "add" if service_name in to_add else "update"
                    logger.error(f"Failed to {action} service {service_name}: {e}")
                    results["failed"].append(f"{action}:{service_name}:{e}")

            # 3. 批量注册到Registry
            if services_to_register:
                await self._batch_register_to_registry(global_agent_store_id, services_to_register)

            # 4. 🔧 新增：触发缓存到文件的异步持久化
            if services_to_register:
                await self._trigger_cache_persistence()
            
            return results

        except Exception as e:
            logger.error(f"Error syncing main client services: {e}")
            raise

    def _get_current_global_agent_store_services(self) -> Dict[str, Any]:
        """获取当前global_agent_store的服务配置"""
        try:
            global_agent_store_id = self.orchestrator.client_manager.global_agent_store_id
            client_ids = self.orchestrator.client_manager.get_agent_clients(global_agent_store_id)

            current_services = {}
            for client_id in client_ids:
                client_config = self.orchestrator.client_manager.get_client_config(client_id)
                if client_config and "mcpServers" in client_config:
                    current_services.update(client_config["mcpServers"])

            return current_services

        except Exception as e:
            logger.error(f"Error getting current main client services: {e}")
            return {}

    async def _remove_service_from_global_agent_store(self, service_name: str) -> bool:
        """从global_agent_store移除服务"""
        try:
            global_agent_store_id = self.orchestrator.client_manager.global_agent_store_id

            # 查找包含该服务的client_ids
            matching_clients = self.orchestrator.client_manager.find_clients_with_service(
                global_agent_store_id, service_name
            )

            # 移除包含该服务的clients
            for client_id in matching_clients:
                self.orchestrator.client_manager._remove_client_and_mapping(global_agent_store_id, client_id)
                logger.debug(f"Removed client {client_id} containing service {service_name}")

            # 从Registry移除
            if hasattr(self.orchestrator.registry, 'remove_service'):
                self.orchestrator.registry.remove_service(global_agent_store_id, service_name)

            return len(matching_clients) > 0

        except Exception as e:
            logger.error(f"Error removing service {service_name} from main client: {e}")
            return False

    async def _batch_register_to_registry(self, agent_id: str, services_to_register: Dict[str, Any]):
        """批量注册服务到Registry"""
        try:
            if not services_to_register:
                return

            logger.debug(f"Batch registering {len(services_to_register)} services to Registry")

            # 获取对应的client_ids
            client_ids = self.orchestrator.client_manager.get_agent_clients(agent_id)

            for client_id in client_ids:
                client_config = self.orchestrator.client_manager.get_client_config(client_id)
                if not client_config:
                    continue

                # 检查这个client是否包含要注册的服务
                client_services = client_config.get("mcpServers", {})
                services_in_client = set(client_services.keys()) & set(services_to_register.keys())

                if services_in_client:
                    try:
                        # 🔧 重构：使用统一的add_service方法而不是register_json_services
                        if hasattr(self.orchestrator, 'store') and self.orchestrator.store:
                            # 使用统一注册架构
                            await self.orchestrator.store.for_store().add_service_async(client_config, source="auto_startup")
                            logger.debug(f"Registered client {client_id} with services: {list(services_in_client)} via unified add_service")
                        else:
                            # 回退到原有方法（带警告）
                            logger.warning("Store reference not available, falling back to register_json_services")
                            await self.orchestrator.register_json_services(client_config, client_id=client_id)
                            logger.debug(f"Registered client {client_id} with services: {list(services_in_client)}")
                    except Exception as e:
                        logger.error(f"Failed to register client {client_id}: {e}")

        except Exception as e:
            logger.error(f"Error in batch register to registry: {e}")

    async def _add_service_to_cache_mapping(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> bool:
        """
        将服务添加到缓存映射（Registry中的两个映射字段）

        缓存映射指的是：
        - registry.agent_clients: Agent-Client映射
        - registry.client_configs: Client配置映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置

        Returns:
            是否成功添加到缓存映射
        """
        try:
            # 生成或获取client_id
            client_id = self.orchestrator.client_manager.generate_client_id()

            # 获取Registry实例
            registry = getattr(self.orchestrator, 'registry', None)
            if not registry:
                logger.error("Registry not available")
                return False

            # 更新缓存映射1：Agent-Client映射
            if agent_id not in registry.agent_clients:
                registry.agent_clients[agent_id] = []
            if client_id not in registry.agent_clients[agent_id]:
                registry.agent_clients[agent_id].append(client_id)

            # 更新缓存映射2：Client配置映射
            registry.client_configs[client_id] = {
                "mcpServers": {service_name: service_config}
            }

            logger.debug(f"✅ 缓存映射更新成功: {service_name} -> {client_id}")
            logger.debug(f"   - agent_clients[{agent_id}] 已更新")
            logger.debug(f"   - client_configs[{client_id}] 已更新")
            return True

        except Exception as e:
            logger.error(f"Failed to add service to cache mapping: {e}")
            return False

    async def _trigger_cache_persistence(self):
        """
        触发缓存映射到文件的同步机制

        注意：这里调用的是同步机制（sync_to_client_manager），
        不是异步持久化（_persist_to_files_async）
        """
        try:
            cache_manager = getattr(self.orchestrator, 'cache_manager', None)
            if cache_manager:
                # 调用缓存同步机制：将缓存映射同步到文件
                cache_manager.sync_to_client_manager(self.orchestrator.client_manager)
                logger.debug("✅ 缓存映射同步到文件成功")
            else:
                # 备用方案
                registry = getattr(self.orchestrator, 'registry', None)
                if registry:
                    registry.sync_to_client_manager(self.orchestrator.client_manager)
                    logger.debug("✅ 缓存映射同步到文件成功（备用方案）")
                else:
                    logger.warning("无法触发缓存映射同步：cache_manager和registry都不可用")

        except Exception as e:
            logger.error(f"Failed to trigger cache mapping sync: {e}")

    async def manual_sync(self) -> Dict[str, Any]:
        """手动触发同步（用于API调用）"""
        logger.info("Manual sync triggered")
        return await self.sync_global_agent_store_from_mcp_json()

    def get_sync_status(self) -> Dict[str, Any]:
        """获取同步状态信息"""
        return {
            "is_running": self.is_running,
            "mcp_json_path": self.mcp_json_path,
            "last_change_time": self.last_change_time,
            "sync_lock_locked": self.sync_lock.locked(),
            "file_observer_running": self.file_observer is not None and self.file_observer.is_alive() if self.file_observer else False
        }
