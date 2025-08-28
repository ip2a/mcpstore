"""
Market manager
市场管理器 - 市场功能的核心管理类
"""

import logging
from typing import Optional, Dict, Any, List
from pathlib import Path

from mcpstore.core.utils.async_sync_helper import get_global_helper
from .service import MarketService
from .converter import MarketConfigConverter
from .types import MarketServerInfo, MarketInstallResult, MCPStoreServiceConfig

logger = logging.getLogger(__name__)


class MarketManager:
    """市场管理器 - 提供完整的市场功能"""

    def __init__(self, data_file_path: Optional[str] = None):
        """
        初始化市场管理器

        Args:
            data_file_path: 市场数据文件路径，默认使用内置路径
        """
        self.logger = logger
        self._sync_helper = get_global_helper()

        # 初始化组件
        self.market_service = MarketService(data_file_path)
        self.config_converter = MarketConfigConverter()

        self.logger.info("MarketManager initialized successfully")
        # 远程来源配置与刷新状态
        self._remote_sources: list[str] = []
        self._last_refresh_ts: float | None = None
        self._is_refreshing: bool = False

        # 磁盘缓存路径
        self._cache_dir = Path(__file__).parent.parent.parent / "data" / "cache" / "market"
        self._remote_cache_file = self._cache_dir / "remote_servers.json"

        # 启动时加载远程缓存
        self._load_remote_cache()


    def get_market_service_config(self,
                                 service_name: str,
                                 user_env: Optional[Dict[str, str]] = None,
                                 preferred_installation: Optional[str] = None) -> MCPStoreServiceConfig:
        """
        获取市场服务的MCPStore配置

        Args:
            service_name: 市场服务名称
            user_env: 用户提供的环境变量
            preferred_installation: 优先使用的安装方式

        Returns:
            MCPStoreServiceConfig: MCPStore服务配置

        Raises:
            ValueError: 当服务不存在或转换失败时
        """

        # 获取市场服务信息
        market_info = self.market_service.get_service(service_name)
        if not market_info:
            available_services = self.get_available_service_names()[:10]  # 显示前10个作为提示
            raise ValueError(
                f"Market service '{service_name}' not found. "
                f"Available services include: {', '.join(available_services)}"
            )

        # 转换配置
        result = self.config_converter.convert_market_to_mcpstore(
            market_info, user_env, preferred_installation
        )

        if not result.success:
            raise ValueError(f"Failed to convert market service '{service_name}': {result.error_message}")

        # 记录警告
        if result.warnings:
            for warning in result.warnings:
                self.logger.warning(f"Market service '{service_name}': {warning}")

        return result.service_config

    async def get_market_service_config_async(self,
                                            service_name: str,
                                            user_env: Optional[Dict[str, str]] = None,
                                            preferred_installation: Optional[str] = None) -> MCPStoreServiceConfig:
        """
        异步获取市场服务的MCPStore配置

        Args:
            service_name: 市场服务名称
            user_env: 用户提供的环境变量
            preferred_installation: 优先使用的安装方式

        Returns:
            MCPStoreServiceConfig: MCPStore服务配置
        """

        # 直接运行同步方法，因为数据操作不需要异步
        return self.get_market_service_config(
            service_name,
            user_env,
            preferred_installation
        )

    def get_market_service_info(self, service_name: str) -> Optional[MarketServerInfo]:
        """
        获取市场服务的详细信息

        Args:
            service_name: 服务名称

        Returns:
            Optional[MarketServerInfo]: 服务信息，不存在则返回None
        """
        return self.market_service.get_service(service_name)

    def search_market_services(self,
                              query: Optional[str] = None,
                              categories: Optional[List[str]] = None,
                              tags: Optional[List[str]] = None,
                              is_official: Optional[bool] = None,
                              limit: int = 20) -> List[MarketServerInfo]:
        """
        搜索市场服务

        Args:
            query: 搜索关键词
            categories: 分类过滤
            tags: 标签过滤
            is_official: 是否只显示官方服务
            limit: 结果数量限制

        Returns:
            List[MarketServerInfo]: 搜索结果
        """

        from .types import MarketSearchFilter

        search_filter = MarketSearchFilter(
            query=query,
            categories=categories or [],
            tags=tags or [],
            is_official=is_official,
            limit=limit
        )

        return self.market_service.list_services(search_filter)

    def get_available_service_names(self) -> List[str]:
        """
        获取所有可用的服务名称

        Returns:
            List[str]: 服务名称列表
        """
        return sorted(list(self.market_service._market_data.keys()))

    def get_categories(self) -> List[str]:
        """
        获取所有可用的分类

        Returns:
            List[str]: 分类列表
        """
        return self.market_service.get_categories()

    def get_tags(self) -> List[str]:
        """
        获取所有可用的标签

        Returns:
            List[str]: 标签列表
        """
        return self.market_service.get_tags()

    def get_market_statistics(self) -> Dict[str, Any]:
        """
        获取市场统计信息

        Returns:
            Dict[str, Any]: 统计信息
        """
        return self.market_service.get_statistics()

    def get_service_installation_info(self, service_name: str) -> Optional[Dict[str, Any]]:
        """
        获取服务的安装信息

        Args:
            service_name: 服务名称

        Returns:
            Optional[Dict[str, Any]]: 安装信息，不存在则返回None
        """

        market_info = self.market_service.get_service(service_name)
        if not market_info:
            return None

        return self.config_converter.get_installation_info(market_info)

    def validate_service_name(self, service_name: str) -> bool:
        """
        验证服务名称是否存在

        Args:
            service_name: 服务名称

        Returns:
            bool: 是否存在
        """
        return service_name in self.market_service._market_data

    def get_recommended_services(self, category: Optional[str] = None, limit: int = 10) -> List[MarketServerInfo]:
        """
        获取推荐的服务列表

        Args:
            category: 限制在指定分类
            limit: 结果数量限制

        Returns:
            List[MarketServerInfo]: 推荐服务列表
        """

        # 优先推荐官方服务
        if category:
            services = self.market_service.get_services_by_category(category)
        else:
            services = list(self.market_service._market_data.values())
    # === Remote refresh & merge (optional) ===
    def add_remote_source(self, url: str):
        """Add a remote servers.json source URL (no validation)."""
        if not isinstance(url, str) or not url:
            return
        if url not in self._remote_sources:
            self._remote_sources.append(url)
            self.logger.info(f"Added remote market source: {url}")

    async def refresh_from_remote_async(self, force: bool = False) -> bool:
        """Fetch servers.json from remote sources and merge into in-memory market data.
        - Prefer local entries when name conflicts occur
        - No persistence by default
        Returns True if at least one source merged successfully
        """
        import asyncio, time, json
        from urllib.request import urlopen

        if self._is_refreshing:
            self.logger.debug("Market remote refresh already in progress")
            return False

        # simple throttle: 12h
        if not force and self._last_refresh_ts and time.time() - self._last_refresh_ts < 12 * 3600:
            self.logger.debug("Market remote refresh throttled (<12h)")
            return False

        if not self._remote_sources:
            self.logger.debug("No remote market sources configured")
            return False

        self._is_refreshing = True
        merged_any = False
        try:
            for url in list(self._remote_sources):
                try:
                    self.logger.info(f"Refreshing market from remote: {url}")
                    # run blocking IO in thread pool
                    def _fetch():
                        with urlopen(url, timeout=10) as resp:
                            data = resp.read()
                            return json.loads(data.decode(resp.headers.get_content_charset() or "utf-8"))
                    raw = await asyncio.to_thread(_fetch)
                    if isinstance(raw, dict):
                        self._merge_market_dict(raw, prefer_local=True)
                        self._save_remote_cache(raw)
                        merged_any = True
                except Exception as e:
                    self.logger.warning(f"Failed to refresh market from {url}: {e}")
                    continue
            if merged_any:
                self._last_refresh_ts = time.time()
        finally:
            self._is_refreshing = False
        return merged_any

    def _merge_market_dict(self, raw: Dict[str, Any], prefer_local: bool = True):
        """Merge a servers dict into MarketService in-memory data; prefer_local keeps existing entries."""
        try:
            # Iterate and map minimally into MarketServerInfo; rely on converter later
            for name, srv in raw.items():
                if prefer_local and self.market_service.get_service(name):
                    continue
                try:
                    from .types import MarketServerInfo
                    info = MarketServerInfo(
                        name=name,
                        description=srv.get("description", ""),
                        homepage=srv.get("homepage"),
                        repo=srv.get("repo"),
                        categories=srv.get("categories", []) or [],
                        tags=srv.get("tags", []) or [],
                        is_official=bool(srv.get("is_official", False)),
                        installations=srv.get("installations", []),
                    )
                    # Inject into current service memory
                    self.market_service._market_data[name] = info
                except Exception as e:
                    self.logger.warning(f"Skip invalid market entry {name}: {e}")
        except Exception as e:
            self.logger.error(f"Failed to merge market dict: {e}")
    def _load_remote_cache(self):
        """加载磁盘远程缓存（如果存在）。"""
        try:
            if self._remote_cache_file.exists():
                import json
                with open(self._remote_cache_file, "r", encoding="utf-8") as f:
                    raw = json.load(f)
                if isinstance(raw, dict):
                    self._merge_market_dict(raw, prefer_local=True)
                    self.logger.info(f"Loaded remote market cache: {self._remote_cache_file}")
        except Exception as e:
            self.logger.debug(f"Failed to load remote market cache: {e}")

    def _save_remote_cache(self, raw: Dict[str, Any]):
        """保存远程数据到磁盘缓存。"""
        try:
            self._cache_dir.mkdir(parents=True, exist_ok=True)
            import json
            with open(self._remote_cache_file, "w", encoding="utf-8") as f:
                json.dump(raw, f, ensure_ascii=False, indent=2)
            self.logger.info(f"Saved remote market cache: {self._remote_cache_file}")
        except Exception as e:
            self.logger.debug(f"Failed to save remote market cache: {e}")

    def get_popular_categories(self, limit: int = 10) -> List[Dict[str, Any]]:
        """
        获取热门分类

        Args:
            limit: 分类数量限制

        Returns:
            List[Dict[str, Any]]: 分类信息列表
        """

        # 统计每个分类的服务数量
        category_counts = {}
        for service in self.market_service._market_data.values():
            for category in service.categories:
                category_counts[category] = category_counts.get(category, 0) + 1

        # 按服务数量排序
        sorted_categories = sorted(
            category_counts.items(),
            key=lambda x: x[1],
            reverse=True
        )

        return [
            {"name": category, "service_count": count}
            for category, count in sorted_categories[:limit]
        ]
