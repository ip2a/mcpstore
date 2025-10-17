"""
设置管理器模块（最新：单一路径）
负责处理 MCPStore 的统一初始化逻辑
"""

import logging
import os
from hashlib import sha1
from typing import Optional, Dict, Any
from copy import deepcopy

logger = logging.getLogger(__name__)


class StoreSetupManager:
    """设置管理器 - 仅保留单一的 setup_store 接口"""

    @staticmethod
    def setup_store(
        mcp_json: str | None = None,
        debug: bool | str = False,
        external_db: Optional[Dict[str, Any]] = None,
        static_config: Optional[Dict[str, Any]] = None,
    ):
        """
        统一初始化 MCPStore（无隐式后台副作用）
        Args:
            mcp_json: mcp.json 文件路径；None 则使用默认
            debug: False=OFF（完全静默）；True=DEBUG；字符串=对应等级
            external_db: 外挂数据库模块配置字典（当前仅支持 cache.redis）
            static_config: 静态配置注入（monitoring/network/features/local_service）
        """
        # 1) 日志
        from mcpstore.config.config import LoggingConfig
        LoggingConfig.setup_logging(debug=debug)

        # 2) 数据空间 & 配置
        from mcpstore.config.json_config import MCPConfig
        from mcpstore.core.store.data_space_manager import DataSpaceManager

        if mcp_json:
            dsm = DataSpaceManager(mcp_json)
            if not dsm.initialize_workspace():
                raise RuntimeError(f"Failed to initialize workspace for: {mcp_json}")
            config = MCPConfig(json_path=mcp_json)
            workspace_dir = str(dsm.workspace_dir)
        else:
            dsm = None
            config = MCPConfig()
            workspace_dir = None

        # 3) 注入静态配置（仅注入，不启动后台）
        base_cfg = config.load_config()
        stat = static_config or {}
        # 映射 network.http_timeout_seconds -> timing.http_timeout_seconds（orchestrator依赖该字段）
        timing = {}
        try:
            http_timeout = stat.get("network", {}).get("http_timeout_seconds")
            if http_timeout is not None:
                timing["http_timeout_seconds"] = int(http_timeout)
        except Exception:
            pass
        if timing:
            base_cfg.setdefault("timing", {}).update(timing)
        # 直接注入其他配置段，供后续模块使用
        for key in ("monitoring", "network", "features", "local_service"):
            if key in stat and isinstance(stat[key], dict):
                base_cfg[key] = deepcopy(stat[key])

        # 若指定了本地服务工作目录，则设置适配器工作目录
        if stat.get("local_service", {}).get("work_dir"):
            from mcpstore.core.integration.local_service_adapter import set_local_service_manager_work_dir
            set_local_service_manager_work_dir(stat["local_service"]["work_dir"])
        elif workspace_dir:
            from mcpstore.core.integration.local_service_adapter import set_local_service_manager_work_dir
            set_local_service_manager_work_dir(workspace_dir)

        # 4) 注册表与缓存后端
        from mcpstore.core.registry import ServiceRegistry
        registry = ServiceRegistry()
        cache_mod = (external_db or {}).get("cache") if isinstance(external_db, dict) else None
        if isinstance(cache_mod, dict) and cache_mod.get("type") == "redis":
            # Redis backend configuration (Fail-Fast on connection errors)
            cache_cfg = {
                "backend": "redis",
                "redis": {
                    "url": cache_mod.get("url"),
                    "password": cache_mod.get("password"),
                    "namespace": cache_mod.get("namespace"),  # Optional, auto-generated if None
                    "socket_timeout": cache_mod.get("socket_timeout"),
                    "healthcheck_interval": cache_mod.get("healthcheck_interval"),
                    "max_connections": cache_mod.get("max_connections"),
                    "_mcp_json_path": getattr(config, "json_path", None),  # For namespace auto-generation
                },
            }
            registry.configure_cache_backend(cache_cfg)

        # 5) 编排器
        from mcpstore.core.orchestrator import MCPOrchestrator
        orchestrator = MCPOrchestrator(base_cfg, registry, mcp_config=config)

        # 6) 实例化 Store（固定组合类）
        from mcpstore.core.store.composed_store import MCPStore as _MCPStore
        store = _MCPStore(orchestrator, config)
        if dsm:
            store._data_space_manager = dsm

        # 7) 同步初始化 orchestrator（无后台副作用）
        from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
        helper = AsyncSyncHelper()
        helper.run_async(orchestrator.setup(), force_background=False)

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
        }
        try:
            setattr(store, "_setup_snapshot", snapshot)
        except Exception:
            pass

        return store
