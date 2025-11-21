"""
Setup Manager module (latest: single path)
Handles unified initialization logic for MCPStore
"""

import logging
import os
from hashlib import sha1
from typing import Optional, Dict, Any, Union
from copy import deepcopy

logger = logging.getLogger(__name__)


class StoreSetupManager:
    """Setup Manager - Keep only single setup_store interface"""

    @staticmethod
    def _generate_namespace(mcpjson_path: str) -> str:
        """
        Automatically generate namespace based on mcp.json path

        Args:
            mcpjson_path: mcp.json file path

        Returns:
            Generated namespace string
        """
        # Use SHA1 hash of the absolute path to generate a unique namespace
        abs_path = os.path.abspath(mcpjson_path)
        hash_obj = sha1(abs_path.encode('utf-8'))
        hash_hex = hash_obj.hexdigest()[:12]  # Use first 12 characters
        return f"mcpstore_{hash_hex}"

    @staticmethod
    def setup_store(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]] = None,
        external_db: Optional[Dict[str, Any]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        **deprecated_kwargs,
    ):
        """
        Unified MCPStore initialization (no implicit background side effects)
        Args:
            mcpjson_path: mcp.json file path; None uses default
            debug: False=OFF (completely silent); True=DEBUG; string=corresponding level
            cache: Cache configuration object (MemoryConfig or RedisConfig), default None (use MemoryConfig)
            external_db: External database module configuration dict (currently only supports cache.redis) - deprecated, use cache parameter
            static_config: Static configuration injection (monitoring/network/features/local_service)
            cache_mode: Cache working mode ("auto" | "local" | "hybrid" | "shared")
                - "auto": Auto detection mode (default)
                - "local": Local mode (JSON + Memory)
                - "hybrid": Hybrid mode (JSON + Redis)
                - "shared": Shared mode (Redis Only)
            **deprecated_kwargs: Historical compatibility parameters (mcp_json / mcp_config_file), will trigger warning
        """
        # Backward-compatible parameter aliases with warnings
        if deprecated_kwargs:
            for _old in ("mcp_json", "mcp_config_file"):
                if _old in deprecated_kwargs:
                    if not mcpjson_path:
                        mcpjson_path = deprecated_kwargs.get(_old)
                    try:
                        import warnings as _warnings
                        _warnings.warn(f"`{_old}` is deprecated; use `mcpjson_path`", DeprecationWarning, stacklevel=2)
                    except Exception:
                        pass
                    logger.warning(f"Parameter `{_old}` is deprecated; use `mcpjson_path`")
        # 1) Logging
        from mcpstore.config.config import LoggingConfig
        LoggingConfig.setup_logging(debug=debug)

        # 2) Data space & configuration
        from mcpstore.config.json_config import MCPConfig
        from mcpstore.config.path_utils import get_user_default_mcp_path
        from mcpstore.core.store.data_space_manager import DataSpaceManager

        # Always use DataSpaceManager for both explicit and default paths
        resolved_mcp_path = mcpjson_path or str(get_user_default_mcp_path())
        
        dsm = DataSpaceManager(resolved_mcp_path)
        if not dsm.initialize_workspace():
            raise RuntimeError(f"Failed to initialize workspace for: {resolved_mcp_path}")
        
        config = MCPConfig(json_path=resolved_mcp_path)
        workspace_dir = str(dsm.workspace_dir)

        # 3) Inject static configuration (only injection, no background startup)
        base_cfg = config.load_config()
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
        from mcpstore.config import (
            MemoryConfig, RedisConfig, detect_strategy, 
            create_kv_store, get_namespace, start_health_check
        )
        
        # Handle cache configuration
        # Priority: cache parameter > external_db (deprecated) > default MemoryConfig
        if cache is None:
            # Check if external_db is provided (deprecated path)
            cache_mod = (external_db or {}).get("cache") if isinstance(external_db, dict) else None
            if isinstance(cache_mod, dict) and cache_mod.get("type") == "redis":
                # Convert old external_db format to new RedisConfig
                logger.warning(
                    "external_db parameter is deprecated. "
                    "Please use cache=RedisConfig(...) instead."
                )
                cache = RedisConfig(
                    url=cache_mod.get("url"),
                    password=cache_mod.get("password"),
                    namespace=cache_mod.get("namespace"),
                    socket_timeout=cache_mod.get("socket_timeout", 5.0),
                    health_check_interval=cache_mod.get("healthcheck_interval", 30),
                    max_connections=cache_mod.get("max_connections", 50),
                )
            else:
                # Default to MemoryConfig
                cache = MemoryConfig()
                logger.debug("Using default MemoryConfig for cache")
        
        # Detect data source strategy
        strategy = detect_strategy(cache, mcpjson_path)
        logger.info(f"Cache initialization: type={cache.cache_type.value}, strategy={strategy.value}")
        
        # Set default namespace for RedisConfig if not provided
        if isinstance(cache, RedisConfig):
            if cache.namespace is None:
                if mcpjson_path:
                    # Auto-generate namespace based on mcp.json path
                    cache.namespace = StoreSetupManager._generate_namespace(mcpjson_path)
                    logger.info(f"Generated namespace from JSON path: {cache.namespace}")
                else:
                    # Use default namespace
                    cache.namespace = "mcpstore"
                    logger.info(f"Using default namespace: {cache.namespace}")
            else:
                logger.info(f"Using user-provided namespace: {cache.namespace}")
        
        # Create KV store using create_kv_store
        kv_store = create_kv_store(cache)
        logger.info(f"Created KV store: {type(kv_store).__name__}")
        
        # Initialize ServiceRegistry with KV store
        registry = ServiceRegistry(kv_store=kv_store)
        
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
        
        # Detect cache mode if set to "auto" (for backward compatibility)
        if cache_mode == "auto":
            # Map strategy to cache_mode
            if strategy.value == "json_memory":
                cache_mode = "local"
            elif strategy.value == "json_custom":
                cache_mode = "hybrid"
            elif strategy.value == "custom_only":
                cache_mode = "shared"
            logger.debug(f"Mapped strategy {strategy.value} to cache_mode: {cache_mode}")

        # 5) 编排器
        from mcpstore.core.orchestrator import MCPOrchestrator
        orchestrator = MCPOrchestrator(base_cfg, registry, mcp_config=config)

        # 6) 实例化 Store（固定组合类）
        from mcpstore.core.store.composed_store import MCPStore as _MCPStore
        store = _MCPStore(orchestrator, config)
        # Always set data space manager since we always create it now
        store._data_space_manager = dsm

        # 7) 同步初始化 orchestrator（无后台副作用）
        from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
        helper = AsyncSyncHelper()
        # 保持后台事件循环常驻，避免组件启动后立即被清理导致状态无法收敛
        helper.run_async(orchestrator.setup(), force_background=True)
        try:
            setattr(store, "_background_helper", helper)
        except Exception:
            pass

        # 8) 可选：预热缓存
        features = stat.get("features", {}) if isinstance(stat, dict) else {}
        if features.get("preload_cache"):
            try:
                helper.run_async(store.initialize_cache_from_files(), force_background=False)
            except Exception as e:
                if features.get("fail_on_cache_preload_error"):
                    raise
                logger.warning(f"Cache preload failed (ignored): {e}")

        # 9) 生成只读配置快照
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
            "external_db": deepcopy(external_db or {}),
            "static_config": deepcopy(stat),
            "cache_config": cache,  # Store cache configuration
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

        return store
