import logging
from typing import Optional

logger = logging.getLogger(__name__)


class ClientManager:
    """
    简化的Client管理器 - 单一数据源架构
    
    在新的架构中，ClientManager只负责提供global_agent_store_id，
    所有的配置和映射关系都通过缓存管理，mcp.json作为唯一持久化数据源。
    
    废弃功能（已移除）：
    - 分片文件操作（client_services.json, agent_clients.json）
    - 客户端配置的文件读写
    - Agent-Client映射的文件管理
    """
    
    def __init__(self, global_agent_store_id: Optional[str] = None):
        """
        初始化客户端管理器

        Args:
            global_agent_store_id: 全局Agent Store ID
        """
        #  单一数据源架构：只需要global_agent_store_id
        self.global_agent_store_id = global_agent_store_id or self._generate_data_space_client_id()
        logger.info(f"ClientManager initialized with global_agent_store_id: {self.global_agent_store_id}")

    def _generate_data_space_client_id(self) -> str:
        """
        生成global_agent_store_id

        Returns:
            str: 固定返回 "global_agent_store"
        """
        # Store级别的Agent固定为global_agent_store
        return "global_agent_store"
