"""
Setup Manager module (latest: single path)
Handles unified initialization logic for MCPStore

This module provides the core setup_store() method that initializes MCPStore
with modern cache configuration (RedisConfig/MemoryConfig).
"""

import logging
import time
import os
from typing import Optional, Dict, Any, Union
from copy import deepcopy

import asyncio
from mcpstore.config.toml_config import init_config

logger = logging.getLogger(__name__)

# 默认 namespace 常量
DEFAULT_NAMESPACE = "mcpstore"


class StoreSetupManager:
    """Setup Manager - Keep only single setup_store interface"""

    @staticmethod
    def setup_store(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
    ):
        """
        Unified MCPStore initialization (no implicit background side effects)

        Args:
            mcpjson_path: mcp.json file path; None uses default (~/.mcpstore/mcp.json)
            debug: False=OFF (completely silent); True=DEBUG; string=corresponding level
            cache: Cache configuration object (MemoryConfig or RedisConfig), default None (use MemoryConfig)
                - MemoryConfig(): In-memory cache (default)
                - RedisConfig(url="redis://localhost:6379/0"): Redis cache
            static_config: Static configuration injection (monitoring/network/features/local_service)
            cache_mode: Cache working mode ("auto" | "local" | "hybrid" | "shared")
                - "auto": Auto detection mode (default, maps to DataSourceStrategy)
                - "local": Local mode (JSON + Memory)
                - "hybrid": Hybrid mode (JSON + Redis)
                - "shared": Shared mode (Redis Only)

        Returns:
            MCPStore: Initialized MCPStore instance

        Raises:
            ValueError: If external_db parameter is provided (no longer supported)
            RuntimeError: If workspace initialization fails

        Examples:
            >>> # Default: Memory cache
            >>> store = MCPStore.setup_store()

            >>> # Redis cache
            >>> from mcpstore.config import RedisConfig
            >>> store = MCPStore.setup_store(
            ...     cache=RedisConfig(url="redis://localhost:6379/0")
            ... )
        """

        # 1) Logging configuration
        from mcpstore.config.config import LoggingConfig
        LoggingConfig.setup_logging(debug=debug)

        # 1.5) Initialize TOML-based global configuration (config.toml + MCPStoreConfig)
        try:
            asyncio.run(init_config())
        except Exception as e:
            logger.warning(f"Failed to initialize TOML configuration system, continuing with defaults: {e}")

        # 2) Data space & configuration
        from mcpstore.config.json_config import MCPConfig
        from mcpstore.config.path_utils import get_user_default_mcp_path
        from mcpstore.core.store.data_space_manager import DataSpaceManager

        # only_db 模式：完全忽略本地 mcp.json，不创建 DataSpace/workspace
        resolved_mcp_path = None
        dsm = None
        config = None
        workspace_dir = None

        if not only_db:
            resolved_mcp_path = mcpjson_path or str(get_user_default_mcp_path())
            dsm = DataSpaceManager(resolved_mcp_path)
            if not dsm.initialize_workspace():
                raise RuntimeError(f"Failed to initialize workspace for: {resolved_mcp_path}")
            config = MCPConfig(json_path=resolved_mcp_path)
            workspace_dir = str(dsm.workspace_dir)
            base_cfg = config.load_config()
        else:
            # 纯 DB 模式：使用空配置，后续仅依赖 static_config 注入
            if mcpjson_path is not None:
                logger.warning("only_db 模式已启用，忽略传入的 mcpjson_path 参数")
            base_cfg = {}

        stat = static_config or {}
        # Map network.http_timeout_seconds -> timing.http_timeout_seconds (orchestrator depends on this field)
        timing = {}
        try:
            http_timeout = stat.get("network", {}).get("http_timeout_seconds")
            if http_timeout is not None:
                timing["http_timeout_seconds"] = int(http_timeout)
        except Exception:
            pass
        if timing:
            base_cfg.setdefault("timing", {}).update(timing)
        # Directly inject other configuration sections for use by subsequent modules
        for key in ("monitoring", "network", "features", "local_service"):
            if key in stat and isinstance(stat[key], dict):
                base_cfg[key] = deepcopy(stat[key])

        # If local service work directory is specified, set adapter work directory
        if stat.get("local_service", {}).get("work_dir"):
            from mcpstore.core.integration.local_service_adapter import set_local_service_manager_work_dir
            set_local_service_manager_work_dir(stat["local_service"]["work_dir"])
        elif workspace_dir:
            from mcpstore.core.integration.local_service_adapter import set_local_service_manager_work_dir
            set_local_service_manager_work_dir(workspace_dir)

        # 4) Registry and cache backend
        from mcpstore.core.registry import ServiceRegistry
        from mcpstore.core.registry.registry_factory import create_registry_from_kv_store
        from mcpstore.config import (
            MemoryConfig, RedisConfig, detect_strategy,
            create_kv_store, get_namespace, start_health_check
        )
        from mcpstore.core.bridge import get_async_bridge
        bridge = get_async_bridge()

        # Handle cache configuration (default to MemoryConfig if not provided)
        if cache is None:
            cache = MemoryConfig()
            logger.debug("Using default MemoryConfig for cache")

        # Detect data source strategy based on cache type and JSON path
        strategy = detect_strategy(cache, mcpjson_path)
        logger.info(f"Cache initialization: type={cache.cache_type.value}, strategy={strategy.value}")
        
        # 设置 namespace：统一使用默认值 "mcpstore"，用户可通过 RedisConfig.namespace 覆盖
        namespace = DEFAULT_NAMESPACE
        if isinstance(cache, RedisConfig):
            if cache.namespace is None:
                cache.namespace = DEFAULT_NAMESPACE
                logger.info(f"Using default namespace: {cache.namespace}")
            else:
                namespace = cache.namespace
                logger.info(f"Using user-provided namespace: {cache.namespace}")
        
        # 关键修复：对于 Redis 后端，必须在 AOB 的后台事件循环中创建 KV store
        # 这样 Redis 连接就绑定到 AOB 的事件循环，后续所有操作都在同一个事件循环中执行
        if isinstance(cache, RedisConfig):
            # 在 AOB 的后台事件循环中创建 Redis KV store
            async def _create_redis_kv_store():
                from key_value.aio.stores.redis import RedisStore
                namespace = get_namespace(cache)
                
                if cache.client:
                    logger.debug(f"Creating RedisStore with user-provided client in AOB loop, namespace={namespace}")
                    return RedisStore(client=cache.client, default_collection=namespace)
                elif cache.url:
                    logger.debug(f"Creating RedisStore with URL in AOB loop, namespace={namespace}")
                    return RedisStore(url=cache.url, default_collection=namespace)
                else:
                    logger.debug(f"Creating RedisStore with parameters in AOB loop: host={cache.host}, port={cache.port or 6379}, db={cache.db or 0}, namespace={namespace}")
                    return RedisStore(
                        host=cache.host,
                        port=cache.port or 6379,
                        db=cache.db or 0,
                        password=cache.password,
                        default_collection=namespace
                    )
            
            kv_store = bridge.run(_create_redis_kv_store(), op_name="create_redis_kv_store")
            logger.info(f"Created Redis KV store in AOB event loop: {type(kv_store).__name__}")
        else:
            # 对于 MemoryStore，可以直接创建（不涉及事件循环绑定问题）
            kv_store = create_kv_store(cache)
            logger.info(f"Created KV store: {type(kv_store).__name__}")
        
        # Use factory pattern for zero delegation
        # 传递统一的 namespace 给 registry
        registry = create_registry_from_kv_store(kv_store, test_mode=False, namespace=namespace)

        # [已移除] ConfigSyncManager 配置备份功能
        # 原因: 所有一致性数据统一通过 add_service() 写入三层缓存架构
        # 初始化时的配置备份会导致数据不一致，因此移除
        
        # Track Redis client lifecycle (for cleanup)
        _user_provided_redis_client = None
        _system_created_redis_client = None
        _health_check_task = None
        
        # Start health check for Redis if configured
        if isinstance(cache, RedisConfig):
            # Get the Redis client from the store
            try:
                from key_value.aio.stores.redis import RedisStore
                if isinstance(kv_store, RedisStore):
                    # Access the private _client attribute (py-key-value doesn't expose public client)
                    redis_client = kv_store._client
                    
                    # Track whether client was user-provided or system-created
                    if cache.client is not None:
                        _user_provided_redis_client = redis_client
                        logger.info("Redis connection: using user-provided client (lifecycle managed by user)")
                    else:
                        _system_created_redis_client = redis_client
                        # Log connection details (mask password)
                        conn_info = []
                        if cache.url:
                            # Mask password in URL
                            masked_url = cache.url
                            if '@' in masked_url and '://' in masked_url:
                                parts = masked_url.split('://', 1)
                                if len(parts) == 2 and '@' in parts[1]:
                                    auth_part = parts[1].split('@')[0]
                                    if ':' in auth_part:
                                        masked_url = masked_url.replace(auth_part.split(':')[1], '***')
                            conn_info.append(f"url={masked_url}")
                        else:
                            conn_info.append(f"host={cache.host or 'localhost'}")
                            conn_info.append(f"port={cache.port or 6379}")
                            conn_info.append(f"db={cache.db or 0}")
                        
                        conn_info.append(f"namespace={cache.namespace}")
                        conn_info.append(f"max_connections={cache.max_connections}")
                        logger.info(f"Redis connection established: {', '.join(conn_info)}")
                    
                    # Start health check
                    _health_check_task = start_health_check(cache, redis_client)
                    if _health_check_task:
                        logger.info(
                            f"Redis health check started: interval={cache.health_check_interval}s"
                        )
            except Exception as e:
                logger.error(f"Failed to initialize Redis connection: {e}", exc_info=True)
        
        # 根据策略自动检测缓存模式
        if cache_mode == "auto":
            # 策略到缓存模式的映射
            if strategy.value == "local_memory":
                cache_mode = "local"
            elif strategy.value == "local_db":
                cache_mode = "hybrid"
            elif strategy.value == "only_db":
                cache_mode = "shared"
            logger.debug(f"策略 {strategy.value} 映射到缓存模式: {cache_mode}")

        # 5) 编排器
        from mcpstore.core.orchestrator import MCPOrchestrator

        standalone_config_manager = None
        if only_db:
            # 提供一个最小的内存配置管理器，避免 MCPConfig 回落到文件模式
            class OnlyDBConfigManager:
                def __init__(self):
                    self._services: Dict[str, Any] = {}

                def get_mcp_config(self):
                    return {"mcpServers": {}}

                def get_service_config(self, name):
                    return self._services.get(name)

                def add_service_config(self, name, cfg):
                    self._services[name] = cfg

                def get_all_service_configs(self):
                    return dict(self._services)

            standalone_config_manager = OnlyDBConfigManager()

        orchestrator = MCPOrchestrator(
            base_cfg,
            registry,
            standalone_config_manager=standalone_config_manager,
            mcp_config=config,
        )



        # 6) 实例化 Store（固定组合类）
        from mcpstore.core.store.composed_store import MCPStore as _MCPStore
        store = _MCPStore(orchestrator, config)
        # Always set data space manager since we always create it now
        store._data_space_manager = dsm

        # 7) 同步初始化 orchestrator
        # 关键：对于 Redis 后端，必须使用 AOB 确保在同一个事件循环中执行
        if isinstance(cache, RedisConfig):
            bridge.run(orchestrator.setup(), op_name="orchestrator.setup")
        else:
            asyncio.run(orchestrator.setup())

        # 6.5) 预写核心实体（store / agents）
        async def _seed_core_entities():
            try:
                cache_layer = getattr(registry, "_cache_layer_manager", None)
                if cache_layer is None:
                    logger.warning("[CACHE_SEED] cache_layer_manager not available; skip seeding core entities.")
                    return

                now = int(time.time())
                # seed agent: global_agent_store
                try:
                    global_agent_id = getattr(store.client_manager, "global_agent_store_id", "global_agent_store")
                except Exception:
                    global_agent_id = "global_agent_store"

                agent_exists = await cache_layer.get_entity("agents", global_agent_id)
                if agent_exists is None:
                    await cache_layer.put_entity(
                        "agents",
                        global_agent_id,
                        {
                            "agent_id": global_agent_id,
                            "created_time": now,
                            "last_active": now,
                            "is_global": True,
                        },
                    )

                # seed store entity
                store_key = "mcpstore"
                store_payload = {
                    "store_id": store_key,
                    "namespace": namespace,
                    "cache_mode": cache_mode,
                    "mcpjson_path": resolved_mcp_path,
                    "workspace_dir": workspace_dir,
                    "created_time": now,
                }
                existing_store = await cache_layer.get_entity("store", store_key)
                if existing_store is None:
                    await cache_layer.put_entity("store", store_key, store_payload)
            except Exception as seed_error:
                logger.warning(f"[CACHE_SEED] Failed to seed core entities: {seed_error}")

        async def _backfill_clients_and_metadata():
            try:
                cache_layer = getattr(registry, "_cache_layer_manager", None)
                if cache_layer is None:
                    logger.warning("[CACHE_SEED] cache_layer_manager not available; skip backfill.")
                    return
                services = await cache_layer.get_all_entities_async("services")
                relations = await cache_layer.get_all_relations_async("agent_services")

                # 构建 service_global_name -> client_id 映射
                client_by_service: dict[str, str] = {}
                agent_by_service: dict[str, str] = {}
                for agent_id, rel in relations.items():
                    for svc in rel.get("services", []) if isinstance(rel, dict) else []:
                        sg = svc.get("service_global_name")
                        cid = svc.get("client_id")
                        if sg:
                            client_by_service[sg] = cid
                            agent_by_service[sg] = agent_id

                from mcpstore.core.utils.id_generator import ClientIDGenerator
                now_ts = int(time.time())

                for sg, data in services.items():
                    if not isinstance(data, dict):
                        continue
                    agent_id = data.get("source_agent") or agent_by_service.get(sg) or "global_agent_store"
                    client_id = client_by_service.get(sg)
                    if not client_id:
                        client_id = ClientIDGenerator.generate_deterministic_id(
                            agent_id=agent_id,
                            service_name=data.get("service_original_name", sg),
                            service_config=data.get("config", {}),
                            global_agent_store_id="global_agent_store",
                        )

                    # backfill clients 实体
                    client_entity = await cache_layer.get_entity("clients", client_id)
                    if not isinstance(client_entity, dict):
                        client_entity = {
                            "client_id": client_id,
                            "agent_id": agent_id,
                            "services": [],
                            "created_time": now_ts,
                        }
                    services_list = client_entity.get("services") or []
                    if sg not in services_list:
                        services_list.append(sg)
                    client_entity.update({
                        "agent_id": agent_id,
                        "services": services_list,
                        "updated_time": now_ts,
                    })
                    await cache_layer.put_entity("clients", client_id, client_entity)

                    # backfill service_metadata 状态
                    existing_meta = await cache_layer.get_state("service_metadata", sg)
                    if existing_meta is None:
                        metadata_state = {
                            "service_global_name": sg,
                            "agent_id": agent_id,
                            "created_time": now_ts,
                            "state_entered_time": now_ts,
                            "reconnect_attempts": 0,
                            "last_ping_time": None,
                        }
                        await cache_layer.put_state("service_metadata", sg, metadata_state)
            except Exception as bf_error:
                logger.warning(f"[CACHE_SEED] Backfill clients/metadata failed: {bf_error}")

        try:
            if isinstance(cache, RedisConfig):
                bridge.run(_seed_core_entities(), op_name="cache.seed_core_entities")
            else:
                asyncio.run(_seed_core_entities())
        except Exception as seed_outer_error:
            logger.warning(f"[CACHE_SEED] Seeding core entities failed: {seed_outer_error}")

        try:
            if isinstance(cache, RedisConfig):
                bridge.run(_backfill_clients_and_metadata(), op_name="cache.backfill_clients_metadata")
            else:
                asyncio.run(_backfill_clients_and_metadata())
        except Exception as bf_outer_error:
            logger.warning(f"[CACHE_SEED] Backfill clients/metadata failed: {bf_outer_error}")

        # [已移除] Phase 11: 配置同步 (sync_json_to_cache)
        # 原因: 所有一致性数据统一通过 add_service() 写入三层缓存架构
        # mcp.json 配置在服务连接时通过 add_service() 写入实体层/关系层/状态层

        # 8) 可选：预热缓存
        features = stat.get("features", {}) if isinstance(stat, dict) else {}
        if features.get("preload_cache"):
            try:
                if isinstance(cache, RedisConfig):
                    bridge.run(store.initialize_cache_from_files(), op_name="store.initialize_cache_from_files")
                else:
                    asyncio.run(store.initialize_cache_from_files())
            except Exception as e:
                if features.get("fail_on_cache_preload_error"):
                    raise
                logger.warning(f"Cache preload failed (ignored): {e}")

        # 9) Generate read-only configuration snapshot
        try:
            lvl = logging.getLogger().getEffectiveLevel()
            if lvl <= logging.DEBUG:
                level_name = "DEBUG"
            elif lvl <= logging.INFO:
                level_name = "INFO"
            elif lvl <= logging.WARNING:
                level_name = "WARNING"
            elif lvl <= logging.ERROR:
                level_name = "ERROR"
            elif lvl <= logging.CRITICAL:
                level_name = "CRITICAL"
            else:
                level_name = "OFF"
        except Exception:
            level_name = "OFF"

        snapshot = {
            "mcp_json": getattr(config, "json_path", None),
            "debug_level": level_name,
            "static_config": deepcopy(stat),
            "cache_config": cache,  # Store cache configuration object
        }
        try:
            setattr(store, "_setup_snapshot", snapshot)
        except Exception:
            pass
        
        # 10) Store Redis client lifecycle tracking for cleanup
        try:
            setattr(store, "_user_provided_redis_client", _user_provided_redis_client)
            setattr(store, "_system_created_redis_client", _system_created_redis_client)
            setattr(store, "_health_check_task", _health_check_task)
        except Exception:
            pass

        # region agent log: debug store setup snapshot
        try:
            import json as _json2
            _log_payload2 = {
                "sessionId": "debug-session",
                "runId": "initial",
                "hypothesisId": "H2",
                "location": "core/store/setup_manager.py:store_setup_done",
                "message": "Store setup completed",
                "data": {
                    "mcp_json": snapshot.get("mcp_json"),
                    "debug_level": snapshot.get("debug_level"),
                    "cache_type": getattr(cache, "cache_type", None).value if getattr(cache, "cache_type", None) is not None else None,
                },
                "timestamp": __import__("time").time(),
            }
            with open("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log", "a", encoding="utf-8") as _f2:
                _f2.write(_json2.dumps(_log_payload2, ensure_ascii=False) + "\n")
        except Exception:
            pass
        # endregion agent log

        return store
