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
import inspect
from typing import Dict, Any

from watchdog.events import FileSystemEventHandler
from watchdog.observers import Observer

from mcpstore.core.bridge import get_async_bridge

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
            logger.info(f"MCP config file modified: {event.src_path}")
            bridge_loop = getattr(self.sync_manager, "bridge_loop", None)
            if not bridge_loop or not bridge_loop.is_running():
                logger.error("[FILE_WATCH] Bridge loop unavailable or not running; abort scheduling")
                return
            asyncio.run_coroutine_threadsafe(self.sync_manager.on_file_changed(), bridge_loop)
            logger.info("[FILE_WATCH] Scheduled on bridge loop")


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
        self._pending_resync = False  # 记录同步期间的新增变更，避免取消正在执行的同步
        self.is_running = False
        # 记录 orchestrator bridge loop，用于跨线程调度（必须可用）
        self._bridge = get_async_bridge()
        self.bridge_loop = self._bridge._ensure_loop()
        
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

            #  执行启动时同步（重放：覆盖同名，不删除其他缓存）
            logger.info("Executing initial replay from mcp.json (upsert only, no delete)")
            await self.sync_global_agent_store_from_mcp_json(delete_missing=False)

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
            
            if self.sync_task and not self.sync_task.done():
                # 同步进行中不取消，记录待重放请求
                self._pending_resync = True
                logger.info("[FILE_WATCH] Sync in progress; mark pending change for replay after current run")
                return

            # 启动防抖同步
            self._pending_resync = False
            self._schedule_sync_task()
            
        except Exception as e:
            logger.error(f"Error handling file change: {e}")

    def _schedule_sync_task(self):
        """统一调度同步任务并附加完成回调"""
        self.sync_task = asyncio.create_task(self._debounced_sync())
        self.sync_task.add_done_callback(self._on_sync_task_done)

    def _on_sync_task_done(self, task: asyncio.Task):
        """同步任务完成后的收尾与重放"""
        try:
            task.result()
        except asyncio.CancelledError:
            logger.debug("Debounced sync task cancelled")
            return
        except Exception as exc:
            logger.error(f"Debounced sync task failed: {exc}", exc_info=True)

        # 如果同步期间有新增变更，自动重放一次
        if self._pending_resync and self.is_running:
            logger.info("[FILE_WATCH] Pending change detected during sync; scheduling replay")
            self._pending_resync = False
            self._schedule_sync_task()
            
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
            
    async def sync_global_agent_store_from_mcp_json(self, *, delete_missing: bool = True):
        """从mcp.json同步global_agent_store（核心方法）"""
        async with self.sync_lock:
            try:
                #  新增：检查同步频率，避免过度同步
                current_time = time.time()

                if self.last_sync_time and (current_time - self.last_sync_time) < self.min_sync_interval:
                    logger.debug(f"Sync skipped due to frequency limit (last sync {current_time - self.last_sync_time:.1f}s ago)")
                    return {"skipped": True, "reason": "frequency_limit"}

                logger.info("Starting global_agent_store sync from mcp.json")

                config = self.orchestrator.mcp_config.load_config()
                services = config.get("mcpServers", {})

                sync_manager = getattr(self.orchestrator, "config_sync_manager", None)
                if sync_manager is not None:
                    try:
                        await sync_manager.sync_json_to_cache(self.mcp_json_path, overwrite=True)
                    except Exception as e:
                        logger.warning(f"JSON to KV sync failed during global sync: {e}")

                logger.debug(f"Found {len(services)} services in mcp.json")

                # 执行同步
                results = await self._sync_global_agent_store_services(services, delete_missing=delete_missing)

                #  新增：记录同步时间
                self.last_sync_time = current_time

                logger.info(f"Global agent store sync completed: {results}")
                return results

            except Exception as e:
                logger.error(f"Global agent store sync failed: {e}")
                raise
                
    async def _sync_global_agent_store_services(self, target_services: Dict[str, Any], *, delete_missing: bool) -> Dict[str, Any]:
        """同步global_agent_store的服务"""
        try:
            global_agent_store_id = self.orchestrator.client_manager.global_agent_store_id

            # 获取当前global_agent_store的服务
            current_services = await self._get_current_global_agent_store_services()
            
            # 计算差异
            current_names = set(current_services.keys())
            target_names = set(target_services.keys())
            
            to_add = target_names - current_names
            to_remove = current_names - target_names if delete_missing else set()
            to_update = target_names & current_names
            
            logger.debug(f"Sync plan: +{len(to_add)} -{len(to_remove)} ~{len(to_update)}")
            logger.info(f"[SYNC_PLAN] add={list(to_add)} remove={list(to_remove)} update={list(to_update)}")
            
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
                    logger.info(f"[SYNC_REMOVE] removing service: {service_name}")
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
                    current_config = current_services.get(service_name, {})
                    target_config = target_services[service_name]

                    if not self._service_config_changed(current_config, target_config):
                        logger.debug(f"Service {service_name} config unchanged, skipping update")
                        continue

                    success = await self._add_service_to_cache_mapping(
                        agent_id=global_agent_store_id,
                        service_name=service_name,
                        service_config=target_config
                    )

                    if success:
                        try:
                            await self.orchestrator.registry.update_service_config_async(service_name, target_config)
                        except Exception as upd_err:
                            logger.error(f"Failed to update service entity config for {service_name}: {upd_err}")
                            results["failed"].append(f"update:{service_name}:config_write")
                            continue

                        services_to_register[service_name] = target_config
                        results["updated"].append(service_name)
                        logger.debug(f"Updated service in cache: {service_name}")
                    else:
                        results["failed"].append(f"update:{service_name}")

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

    async def _get_current_global_agent_store_services(self) -> Dict[str, Any]:
        """获取当前 global_agent_store 的服务配置（以 KV/Registry 为真源）"""
        try:
            agent_id = self.orchestrator.client_manager.global_agent_store_id
            current_services: Dict[str, Any] = {}

            service_entities = await self.orchestrator.registry._cache_layer_manager.get_all_entities_async("services")
            for entity_key, entity_data in service_entities.items():
                if hasattr(entity_data, 'value'):
                    data = entity_data.value
                elif isinstance(entity_data, dict):
                    data = entity_data
                else:
                    continue

                if data.get('source_agent') != agent_id:
                    continue

                service_name = data.get('service_original_name', entity_key)
                config = data.get('config') or {}
                current_services[service_name] = config

            return current_services

        except Exception as e:
            logger.error(f"Error getting current main client services: {e}")
            return {}

    async def _remove_service_from_global_agent_store(self, service_name: str) -> bool:
        """从global_agent_store移除服务"""
        try:
            global_agent_store_id = self.orchestrator.client_manager.global_agent_store_id

            # 查找包含该服务的client_ids（通过 registry 公共接口获取 agent 的 clients 再筛选）
            matching_clients = []
            try:
                client_ids = await self.orchestrator.registry.get_agent_clients_async(global_agent_store_id)
                logger.info(f"[SYNC_REMOVE] clients of {global_agent_store_id}: {client_ids}")
                for cid in client_ids:
                    client_entity = await self.orchestrator.registry.get_client_config_from_cache_async(cid)
                    services = (client_entity.get("services") if isinstance(client_entity, dict) else None) or []
                    if service_name in services:
                        matching_clients.append(cid)
            except Exception as e:
                logger.error(f"Failed to enumerate clients for removal: {e}")

            # 记录客户端映射（不阻断删除，交由 remove_service_async 清理关系）
            if matching_clients:
                logger.info(f"[SYNC_REMOVE] will remove service {service_name} with client mappings {matching_clients}")

            # 从Registry移除（使用异步版本）
            if hasattr(self.orchestrator.registry, 'remove_service_async'):
                logger.info(f"[SYNC_REMOVE] calling registry.remove_service_async agent={global_agent_store_id} service={service_name}")
                registry_obj = self.orchestrator.registry
                remove_attr = getattr(registry_obj, 'remove_service_async', None)
                logger.info(
                    "[SYNC_REMOVE][DIAG] registry=%s remove_service_async=%s is_coro_fn=%s module=%s",
                    type(registry_obj),
                    remove_attr,
                    inspect.iscoroutinefunction(remove_attr),
                    inspect.getmodule(remove_attr).__name__ if remove_attr else None
                )
                try:
                    result = remove_attr(global_agent_store_id, service_name) if remove_attr else None
                    if inspect.iscoroutine(result):
                        await result
                    else:
                        logger.warning(
                            "[SYNC_REMOVE][DIAG] remove_service_async returned non-coroutine: %s (%s)",
                            result, type(result)
                        )
                    logger.info(f"[SYNC_REMOVE] registry.remove_service_async completed agent={global_agent_store_id} service={service_name}")
                except Exception as rm_err:
                    logger.error(f"[SYNC_REMOVE] registry.remove_service_async failed agent={global_agent_store_id} service={service_name}: {rm_err}", exc_info=True)
                    return False

            return True

        except Exception as e:
            logger.error(f"Error removing service {service_name} from main client: {e}")
            return False

    async def _batch_register_to_registry(self, agent_id: str, services_to_register: Dict[str, Any]):
        """批量注册服务到Registry（改进版：避免重复注册）"""
        try:
            if not services_to_register:
                return

            logger.debug(f"Batch registering {len(services_to_register)} services to Registry")
            registered_count = 0
            skipped_count = 0
            event_bus = getattr(getattr(self.orchestrator, "container", None), "_event_bus", None) or getattr(self.orchestrator, "event_bus", None)

            for service_name, config in services_to_register.items():
                if await self.orchestrator.registry.has_service_async(agent_id, service_name):
                    skipped_count += 1
                    continue
                try:
                    if not event_bus:
                        raise RuntimeError("EventBus unavailable for bootstrap registration")

                    from mcpstore.core.events.service_events import ServiceBootstrapRequested
                    from mcpstore.core.utils.id_generator import ClientIDGenerator

                    client_id = ClientIDGenerator.generate_deterministic_id(
                        agent_id=agent_id,
                        service_name=service_name,
                        service_config=config,
                        global_agent_store_id=getattr(self.orchestrator.client_manager, "global_agent_store_id", "global_agent_store")
                    )

                    bootstrap_event = ServiceBootstrapRequested(
                        agent_id=agent_id,
                        service_name=service_name,
                        service_config=config,
                        client_id=client_id,
                        global_name=service_name,
                        origin_agent_id=agent_id,
                        origin_local_name=service_name,
                        source="sync_mcpjson"
                    )
                    await event_bus.publish(bootstrap_event, wait=False)
                    registered_count += 1
                except Exception as e:
                    logger.error(f"Failed to register service {service_name}: {e}")

            logger.info(f"Batch registration completed (bootstrap path): {registered_count} registered, {skipped_count} skipped")

        except Exception as e:
            logger.error(f"Error in batch register to registry: {e}")

    async def _add_service_to_cache_mapping(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> bool:
        """
        将服务添加到缓存映射（Registry中的映射字段）

        注意：
        - registry.agent_clients: 已移除 (Phase 4) - 现在从 pyvk 推导
        - registry.clients: 已移除 (Phase 5) - 现在存储在 pyvk

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
            existing_client_id = await self._find_existing_client_id_for_service_async(agent_id, service_name)

            if existing_client_id:
                # 使用现有的client_id，只更新配置
                client_id = existing_client_id
                logger.debug(f" Using existing client_id: {service_name} -> {client_id}")
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
                logger.debug(f" Generating new client_id: {service_name} -> {client_id}")

            # 更新缓存映射1：Agent-Client映射（通过Registry公共API）
            try:
                registry._agent_client_service.add_agent_client_mapping(agent_id, client_id)
            except Exception as e:
                logger.error(f"Failed to add agent-client mapping for {agent_id}/{client_id}: {e}")
                return False

            # 更新缓存映射2：Client实体（使用 main_registry 格式）
            try:
                # 获取或创建 client 实体（使用 main_registry 格式）
                client_entity = await registry._cache_layer_manager.get_entity("clients", client_id)
                if not isinstance(client_entity, dict):
                    # 创建新的 client 实体
                    client_entity = {
                        "client_id": client_id,
                        "agent_id": agent_id,
                        "services": [],
                        "created_time": int(time.time()),
                    }
                
                # 更新 services 列表
                services = client_entity.get("services") or []
                if service_name not in services:
                    services.append(service_name)
                
                client_entity.update({
                    "agent_id": agent_id,
                    "services": services,
                    "updated_time": int(time.time()),
                })
                
                await registry._cache_layer_manager.put_entity("clients", client_id, client_entity)
            except Exception as e:
                logger.error(f"Failed to create/update client entity for {client_id}: {e}")
                raise  # 按要求抛出错误，不做静默处理

            logger.debug(f"Cache mapping updated successfully: {service_name} -> {client_id}")
            logger.debug(f"   - agent_clients[{agent_id}] updated via Registry API")
            logger.debug(f"   - clients[{client_id}] updated via Registry API")
            return True

        except Exception as e:
            logger.error(f"Failed to add service to cache mapping: {e}")
            return False

    async def _find_existing_client_id_for_service_async(self, agent_id: str, service_name: str) -> str:
        """
        查找指定服务是否已有对应的client_id（异步版本）

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

            # 获取该agent的所有client_id（通过Registry公共API）- 从 pykv 获取
            client_ids = await registry.get_agent_clients_async(agent_id)

            # 遍历每个client_id，检查是否包含目标服务（使用新格式：services 列表）
            for client_id in client_ids:
                client_entity = await registry.get_client_config_from_cache_async(client_id)
                if client_entity and isinstance(client_entity, dict):
                    services = client_entity.get("services", [])
                    if service_name in services:
                        logger.debug(f" Found existing client_id: {service_name} -> {client_id}")
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
