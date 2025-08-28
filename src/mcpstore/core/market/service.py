"""
Market service interface
市场服务接口 - 提供市场数据的查询和搜索功能
"""

import json
import logging
from pathlib import Path
from typing import Dict, List, Optional, Any
from .types import MarketServerInfo, MarketSearchFilter, MarketServerRepository, MarketServerAuthor, MarketInstallation

logger = logging.getLogger(__name__)


class MarketService:
    """市场服务接口"""
    
    def __init__(self, data_file_path: Optional[str] = None):
        """
        初始化市场服务
        
        Args:
            data_file_path: 市场数据文件路径，默认使用内置路径
        """
        self.logger = logger
        self._market_data: Dict[str, MarketServerInfo] = {}
        self._categories: List[str] = []
        self._tags: List[str] = []
        
        # 确定数据文件路径
        if data_file_path:
            self.data_file_path = Path(data_file_path)
        else:
            # 使用默认的内置数据文件
            current_dir = Path(__file__).parent.parent.parent  # src/mcpstore/core -> src/mcpstore
            self.data_file_path = current_dir / "data" / "market" / "servers.json"
        
        # 加载市场数据
        self._load_market_data()
    
    def _load_market_data(self):
        """加载市场数据"""
        try:
            if not self.data_file_path.exists():
                self.logger.warning(f"Market data file not found: {self.data_file_path}")
                return
            
            with open(self.data_file_path, 'r', encoding='utf-8') as f:
                raw_data = json.load(f)
            
            # 转换原始数据为结构化对象
            self._market_data = {}
            categories_set = set()
            tags_set = set()
            
            for service_name, service_data in raw_data.items():
                try:
                    market_info = self._parse_service_data(service_name, service_data)
                    self._market_data[service_name] = market_info
                    
                    # 收集分类和标签
                    categories_set.update(market_info.categories)
                    tags_set.update(market_info.tags)
                    
                except Exception as e:
                    self.logger.warning(f"Failed to parse service {service_name}: {e}")
                    continue
            
            self._categories = sorted(list(categories_set))
            self._tags = sorted(list(tags_set))
            
            self.logger.info(f"Loaded {len(self._market_data)} market services from {self.data_file_path}")
            self.logger.debug(f"Available categories: {len(self._categories)}, tags: {len(self._tags)}")
            
        except Exception as e:
            self.logger.error(f"Failed to load market data: {e}")
            self._market_data = {}
            self._categories = []
            self._tags = []
    
    def _parse_service_data(self, service_name: str, service_data: Dict[str, Any]) -> MarketServerInfo:
        """
        解析单个服务的数据
        
        Args:
            service_name: 服务名称
            service_data: 原始服务数据
            
        Returns:
            MarketServerInfo: 解析后的服务信息
        """
        
        # 解析代码仓库信息
        repo_data = service_data.get("repository", {})
        repository = MarketServerRepository(
            type=repo_data.get("type", "git"),
            url=repo_data.get("url", "")
        )
        
        # 解析作者信息
        author_data = service_data.get("author", {})
        author = MarketServerAuthor(
            name=author_data.get("name", "Unknown")
        )
        
        # 解析安装方式
        installations = {}
        installations_data = service_data.get("installations", {})
        for install_type, install_data in installations_data.items():
            # 验证安装数据的基本结构
            if not isinstance(install_data, dict):
                self.logger.warning(f"Invalid installation data for {service_name}.{install_type}, skipping")
                continue
                
            command = install_data.get("command", "")
            if not command or not isinstance(command, str):
                self.logger.warning(f"Missing or invalid command for {service_name}.{install_type}, skipping")
                continue
                
            installations[install_type] = MarketInstallation(
                type=install_type,
                command=command.strip(),
                args=install_data.get("args", []) if isinstance(install_data.get("args"), list) else [],
                env=install_data.get("env", {}) if isinstance(install_data.get("env"), dict) else {},
                working_dir=install_data.get("working_dir") if install_data.get("working_dir") else None
            )
        
        # 验证是否有有效的安装方式
        if not installations:
            raise ValueError(f"No valid installation methods found for service {service_name}")
        
        # 创建服务信息对象
        market_info = MarketServerInfo(
            name=service_name,
            display_name=service_data.get("display_name", service_name),
            description=service_data.get("description", ""),
            repository=repository,
            homepage=service_data.get("homepage", ""),
            author=author,
            license=service_data.get("license", "Unknown"),
            categories=service_data.get("categories", []) if isinstance(service_data.get("categories"), list) else [],
            tags=service_data.get("tags", []) if isinstance(service_data.get("tags"), list) else [],
            installations=installations,
            is_official=service_data.get("is_official", False)
        )
        
        return market_info
    
    def get_service(self, service_name: str) -> Optional[MarketServerInfo]:
        """
        获取指定的市场服务信息
        
        Args:
            service_name: 服务名称
            
        Returns:
            Optional[MarketServerInfo]: 服务信息，如果不存在则返回None
        """
        return self._market_data.get(service_name)
    
    def list_services(self, search_filter: Optional[MarketSearchFilter] = None) -> List[MarketServerInfo]:
        """
        列出市场服务
        
        Args:
            search_filter: 搜索过滤器
            
        Returns:
            List[MarketServerInfo]: 符合条件的服务列表
        """
        
        services = list(self._market_data.values())
        
        if not search_filter:
            return services
        
        # 应用搜索过滤器
        filtered_services = []
        
        for service in services:
            # 关键词搜索
            if search_filter.query:
                query_lower = search_filter.query.lower()
                searchable_text = " ".join([
                    service.name,
                    service.display_name,
                    service.description,
                    " ".join(service.categories),
                    " ".join(service.tags)
                ]).lower()
                
                if query_lower not in searchable_text:
                    continue
            
            # 分类过滤
            if search_filter.categories:
                if not any(cat in service.categories for cat in search_filter.categories):
                    continue
            
            # 标签过滤
            if search_filter.tags:
                if not any(tag in service.tags for tag in search_filter.tags):
                    continue
            
            # 官方服务过滤
            if search_filter.is_official is not None:
                if service.is_official != search_filter.is_official:
                    continue
            
            filtered_services.append(service)
        
        # 排序：官方服务优先
        filtered_services.sort(key=lambda s: (not s.is_official, s.name))
        
        # 应用数量限制
        if search_filter.limit and search_filter.limit > 0:
            filtered_services = filtered_services[:search_filter.limit]
        
        return filtered_services
    
    def search_services(self, query: str, limit: int = 10) -> List[MarketServerInfo]:
        """
        搜索市场服务
        
        Args:
            query: 搜索关键词
            limit: 结果数量限制
            
        Returns:
            List[MarketServerInfo]: 搜索结果
        """
        
        search_filter = MarketSearchFilter(query=query, limit=limit)
        return self.list_services(search_filter)
    
    def get_services_by_category(self, category: str) -> List[MarketServerInfo]:
        """
        按分类获取服务
        
        Args:
            category: 分类名称
            
        Returns:
            List[MarketServerInfo]: 该分类下的服务列表
        """
        
        search_filter = MarketSearchFilter(categories=[category])
        return self.list_services(search_filter)
    
    def get_services_by_tag(self, tag: str) -> List[MarketServerInfo]:
        """
        按标签获取服务
        
        Args:
            tag: 标签名称
            
        Returns:
            List[MarketServerInfo]: 包含该标签的服务列表
        """
        
        search_filter = MarketSearchFilter(tags=[tag])
        return self.list_services(search_filter)
    
    def get_categories(self) -> List[str]:
        """
        获取所有可用的分类
        
        Returns:
            List[str]: 分类列表
        """
        return self._categories.copy()
    
    def get_tags(self) -> List[str]:
        """
        获取所有可用的标签
        
        Returns:
            List[str]: 标签列表
        """
        return self._tags.copy()
    
    def get_official_services(self) -> List[MarketServerInfo]:
        """
        获取官方服务列表
        
        Returns:
            List[MarketServerInfo]: 官方服务列表
        """
        
        search_filter = MarketSearchFilter(is_official=True)
        return self.list_services(search_filter)
    
    def get_service_count(self) -> int:
        """
        获取市场服务总数
        
        Returns:
            int: 服务总数
        """
        return len(self._market_data)
    
    def get_statistics(self) -> Dict[str, Any]:
        """
        获取市场统计信息
        
        Returns:
            Dict[str, Any]: 统计信息
        """
        
        official_count = len([s for s in self._market_data.values() if s.is_official])
        
        # 统计安装方式
        installation_types = {}
        for service in self._market_data.values():
            for install_type in service.installations.keys():
                installation_types[install_type] = installation_types.get(install_type, 0) + 1
        
        return {
            "total_services": len(self._market_data),
            "official_services": official_count,
            "community_services": len(self._market_data) - official_count,
            "categories_count": len(self._categories),
            "tags_count": len(self._tags),
            "installation_types": installation_types,
            "data_file": str(self.data_file_path)
        }
