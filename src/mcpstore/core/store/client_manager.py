import logging
from typing import Optional

logger = logging.getLogger(__name__)


class ClientManager:
    """
    Simplified Client Manager - Single data source architecture

    In the new architecture, ClientManager is only responsible for providing global_agent_store_id,
    all configurations and mappings are managed through cache, with mcp.json as the only persistence data source.

    Deprecated features (removed):
    - Sharded file operations (client_services.json, agent_clients.json)
    - Client configuration file read/write
    - Agent-Client mapping file management
    """
    
    def __init__(self, global_agent_store_id: Optional[str] = None):
        """
        Initialize client manager

        Args:
            global_agent_store_id: Global Agent Store ID
        """
        # Single data source architecture: only need global_agent_store_id
        self.global_agent_store_id = global_agent_store_id or self._generate_data_space_client_id()
        logger.info(f"ClientManager initialized with global_agent_store_id: {self.global_agent_store_id}")

    def _generate_data_space_client_id(self) -> str:
        """
        Generate global_agent_store_id

        Returns:
            str: Fixed return "global_agent_store"
        """
        # Store-level Agent is fixed to global_agent_store
        return "global_agent_store"
