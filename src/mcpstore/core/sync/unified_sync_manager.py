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
from typing import Dict, Any

# 强制使用内置 watchdog（不再可选）
from mcpstore.utils.watchdog.observers import Observer
from mcpstore.utils.watchdog.events import FileSystemEventHandler

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
        self.last_sync_time = None  #  新增：记录上次同步时间
        self.min_sync_interval = 5.0  #  新增：最小同步间隔（秒）
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

            #  执行启动时同步（始终启用）
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
                # 统一使用全局同步方法
                await self.sync_global_agent_store_from_mcp_json()
                
        except asyncio.CancelledError:
            logger.debug("Debounced sync cancelled")
        except Exception as e:
            logger.error(f"Error in debounced sync: {e}")
            
    async def sync_global_agent_store_from_mcp_json(self):
        """从mcp.json同步global_agent_store（核心方法）"""
        async with self.sync_lock:
            try:
                #  新增：检查同步频率，避免过度同步
                import time
                current_time = time.time()

                if self.last_sync_time and (current_time - self.last_sync_time) < self.min_sync_interval:
                    logger.debug(f"Sync skipped due to frequency limit (last sync {current_time - self.last_sync_time:.1f}s ago)")
                    return {"skipped": True, "reason": "frequency_limit"}

                logger.info("Starting global_agent_store sync from mcp.json")

                # 读取最新配置
                config = self.orchestrator.mcp_config.load_config()
                services = config.get("mcpServers", {})

                logger.debug(f"Found {len(services)} services in mcp.json")

                # 执行同步
                results = await self._sync_global_agent_store_services(services)

                #  新增：记录同步时间
                self.last_sync_time = current_time

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
            
            # 2. 添加/更新服务（改进逻辑：只处理真正需要变更的服务）
            services_to_register = {}

            # 处理新增服务
            for service_name in to_add:
                try:
                    success = await self._add_service_to_cache_mapping(
                        agent_id=global_agent_store_id,
                        service_name=service_name,
                        service_config=target_services[service_name]
                    )

                    if success:
                        services_to_register[service_name] = target_services[service_name]
                        results["added"].append(service_name)
                        logger.debug(f"Added new service to cache: {service_name}")
                    else:
                        results["failed"].append(f"add:{service_name}")

                except Exception as e:
                    logger.error(f"Failed to add service {service_name}: {e}")
                    results["failed"].append(f"add:{service_name}:{e}")

            # 处理更新服务（只有配置真正变化时才更新）
            for service_name in to_update:
                try:
                    # 检查配置是否真的有变化
                    current_config = current_services.get(service_name, {})
                    target_config = target_services[service_name]

                    if self._service_config_changed(current_config, target_config):
                        success = await self._add_service_to_cache_mapping(
                            agent_id=global_agent_store_id,
                            service_name=service_name,
                            service_config=target_config
                        )

                        if success:
                            services_to_register[service_name] = target_config
                            results["updated"].append(service_name)
                            logger.debug(f"Updated service in cache: {service_name}")
                        else:
                            results["failed"].append(f"update:{service_name}")
                    else:
                        logger.debug(f"Service {service_name} config unchanged, skipping update")

                except Exception as e:
                    logger.error(f"Failed to update service {service_name}: {e}")
                    results["failed"].append(f"update:{service_name}:{e}")

            # 3. 批量注册到Registry（只注册真正需要注册的服务）
            if services_to_register:
                logger.info(f"Registering {len(services_to_register)} services to Registry: {list(services_to_register.keys())}")
                await self._batch_register_to_registry(global_agent_store_id, services_to_register)
            else:
                logger.debug("No services need to be registered to Registry")

            # 4.  新增：触发缓存到文件的异步持久化
            if services_to_register:
                await self._trigger_cache_persistence()
            
            return results

        except Exception as e:
            logger.error(f"Error syncing main client services: {e}")
            raise

    def _get_current_global_agent_store_services(self) -> Dict[str, Any]:
        """获取当前global_agent_store的服务配置"""
        try:
            # single-source: derive current services from registry cache only
            agent_id = self.orchestrator.client_manager.global_agent_store_id
            current_services = {}
            for service_name in self.orchestrator.registry.get_all_service_names(agent_id):
                config = self.orchestrator.mcp_config.get_service_config(service_name) or {}
                if config:
                    current_services[service_name] = config

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
        """批量注册服务到Registry（改进版：避免重复注册）"""
        try:
            if not services_to_register:
                return

            logger.debug(f"Batch registering {len(services_to_register)} services to Registry")

            # single-source: register services_to_register directly if not present
            registered_count = 0
            skipped_count = 0

            for service_name, config in services_to_register.items():
                if self.orchestrator.registry.has_service(agent_id, service_name):
                    skipped_count += 1
                    continue
                try:
                    if hasattr(self.orchestrator, 'store') and self.orchestrator.store:
                        # Use existing add_service_async with explicit mcpServers shape (no extra kwargs)
                        await self.orchestrator.store.for_store().add_service_async(
                            config={"mcpServers": {service_name: config}}
                        )
                    else:
                        # Update mcp.json directly then let lifecycle initialize
                        current = self.orchestrator.mcp_config.load_config()
                        m = current.get("mcpServers", {})
                        m[service_name] = config
                        current["mcpServers"] = m
                        self.orchestrator.mcp_config.save_config(current)
                    registered_count += 1
                except Exception as e:
                    logger.error(f"Failed to register service {service_name}: {e}")

            logger.info(f"Batch registration completed: {registered_count} registered, {skipped_count} skipped")

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
            # 获取Registry实例
            registry = getattr(self.orchestrator, 'registry', None)
            if not registry:
                logger.error("Registry not available")
                return False

            #  修复：检查是否已存在该服务的client_id，避免重复生成
            existing_client_id = self._find_existing_client_id_for_service(agent_id, service_name)

            if existing_client_id:
                # 使用现有的client_id，只更新配置
                client_id = existing_client_id
                logger.debug(f" 使用现有client_id: {service_name} -> {client_id}")
            else:
                #  使用统一的ClientIDGenerator生成确定性client_id
                from mcpstore.core.utils.id_generator import ClientIDGenerator
                
                # UnifiedMCPSyncManager主要处理Store级别的服务，所以使用global_agent_store_id
                global_agent_store_id = getattr(self.orchestrator.client_manager, 'global_agent_store_id', 'global_agent_store')
                
                client_id = ClientIDGenerator.generate_deterministic_id(
                    agent_id=agent_id,
                    service_name=service_name,
                    service_config=service_config,
                    global_agent_store_id=global_agent_store_id
                )
                logger.debug(f" 生成新client_id: {service_name} -> {client_id}")

            # 更新缓存映射1：Agent-Client映射
            if agent_id not in registry.agent_clients:
                registry.agent_clients[agent_id] = []
            if client_id not in registry.agent_clients[agent_id]:
                registry.agent_clients[agent_id].append(client_id)

            # 更新缓存映射2：Client配置映射
            registry.client_configs[client_id] = {
                "mcpServers": {service_name: service_config}
            }

            logger.debug(f"缓存映射更新成功: {service_name} -> {client_id}")
            logger.debug(f"   - agent_clients[{agent_id}] 已更新")
            logger.debug(f"   - client_configs[{client_id}] 已更新")
            return True

        except Exception as e:
            logger.error(f"Failed to add service to cache mapping: {e}")
            return False

    def _find_existing_client_id_for_service(self, agent_id: str, service_name: str) -> str:
        """
        查找指定服务是否已有对应的client_id

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            现有的client_id，如果不存在则返回None
        """
        try:
            registry = getattr(self.orchestrator, 'registry', None)
            if not registry:
                return None

            # 获取该agent的所有client_id
            client_ids = registry.agent_clients.get(agent_id, [])

            # 遍历每个client_id，检查是否包含目标服务
            for client_id in client_ids:
                client_config = registry.client_configs.get(client_id, {})
                if service_name in client_config.get("mcpServers", {}):
                    logger.debug(f" 找到现有client_id: {service_name} -> {client_id}")
                    return client_id

            return None

        except Exception as e:
            logger.error(f"Error finding existing client_id for service {service_name}: {e}")
            return None

    def _service_config_changed(self, current_config: Dict[str, Any], target_config: Dict[str, Any]) -> bool:
        """
        检查服务配置是否发生变化

        Args:
            current_config: 当前配置
            target_config: 目标配置

        Returns:
            配置是否发生变化
        """
        try:
            # 简单的字典比较，可以根据需要扩展
            import json
            current_str = json.dumps(current_config, sort_keys=True)
            target_str = json.dumps(target_config, sort_keys=True)
            changed = current_str != target_str

            if changed:
                logger.debug(f"Service config changed: {current_str} -> {target_str}")

            return changed

        except Exception as e:
            logger.error(f"Error comparing service configs: {e}")
            # 出错时保守处理，认为有变化
            return True

    async def _trigger_cache_persistence(self):
        """
        触发缓存映射到文件的同步机制

        注意：这里调用的是同步机制（sync_to_client_manager），
        不是异步持久化（_persist_to_files_async）
        """
        try:
            # 单源模式：不再将缓存映射同步到分片文件
            logger.debug("Single-source mode: skip shard mapping sync (agent_clients/client_services)")
        except Exception as e:
            logger.error(f"Failed in shard sync skip path: {e}")

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

