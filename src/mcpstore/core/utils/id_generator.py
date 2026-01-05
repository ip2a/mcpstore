"""
Client ID Generator Module
Provides unified and deterministic client ID generation for MCPStore
"""

import hashlib
import logging
import random
import string
import uuid
from typing import Dict, Any

logger = logging.getLogger(__name__)


class ClientIDGenerator:
    """
    Unified Client ID Generator

    Provides deterministic client_id generation algorithm, ensuring:
    1. Same input always produces same ID
    2. Different Agent/Service combinations produce different IDs
    3. Supports both Store and Agent modes
    """

    @staticmethod
    def generate_deterministic_id(agent_id: str, service_name: str,
                                  service_config: Dict[str, Any],
                                  global_agent_store_id: str) -> str:
        """
        Generate deterministic client_id

        Args:
            agent_id: Agent ID
            service_name: Service name
            service_config: Service configuration (used to generate hash)
            global_agent_store_id: Global Agent Store ID

        Returns:
            str: Deterministic client_id

        Format description:
        - Store service: client_store_{service_name}_{config_hash}
        - Agent service: client_{agent_id}_{service_name}_{config_hash}
        """
        try:
            # Generate configuration hash (ensure deterministic)
            config_str = str(sorted(service_config.items())) if service_config else ""
            config_hash = hashlib.md5(config_str.encode()).hexdigest()[:8]

            # Generate different format client_id based on agent type
            if agent_id == global_agent_store_id:
                # Store service format
                client_id = f"client_store_{service_name}_{config_hash}"
                logger.debug(f" [ID_GEN] Generated Store client_id: {service_name} -> {client_id}")
            else:
                # Agent service format
                client_id = f"client_{agent_id}_{service_name}_{config_hash}"
                logger.debug(f" [ID_GEN] Generated Agent client_id: {agent_id}:{service_name} -> {client_id}")

            return client_id

        except Exception as e:
            logger.error(f" [ID_GEN] Failed to generate client_id for {agent_id}:{service_name}: {e}")
            # Fallback to simple format
            fallback_id = f"client_{agent_id}_{service_name}_fallback"
            logger.warning(f"[ID_GEN] [WARN] Using fallback client_id: {fallback_id}")
            return fallback_id

    @staticmethod
    def parse_client_id(client_id: str) -> Dict[str, str]:
        """
        解析client_id，提取其中的信息

        Args:
            client_id: Client ID字符串

        Returns:
            Dict: 包含解析结果的字典
            - type: "store" 或 "agent"
            - agent_id: Agent ID（仅Agent类型）
            - service_name: 服务名称
            - config_hash: 配置哈希
        """
        try:
            parts = client_id.split('_')

            if len(parts) >= 3 and parts[0] == "client":
                if parts[1] == "store":
                    # Store格式: client_store_{service_name}_{hash}
                    return {
                        "type": "store",
                        "agent_id": None,
                        "service_name": parts[2],
                        "config_hash": parts[3] if len(parts) > 3 else ""
                    }
                else:
                    # Agent格式: client_{agent_id}_{service_name}_{hash}
                    return {
                        "type": "agent",
                        "agent_id": parts[1],
                        "service_name": parts[2],
                        "config_hash": parts[3] if len(parts) > 3 else ""
                    }


            return {
                "type": "unknown",
                "agent_id": None,
                "service_name": None,
                "config_hash": None
            }

        except Exception as e:
            logger.error(f" [ID_GEN] Error parsing client_id {client_id}: {e}")
            return {
                "type": "error",
                "agent_id": None,
                "service_name": None,
                "config_hash": None
            }

    @staticmethod
    def is_deterministic_format(client_id: str) -> bool:
        """
        检查client_id是否是确定性格式

        Args:
            client_id: Client ID字符串

        Returns:
            bool: 是否是确定性格式
        """
        try:
            parsed = ClientIDGenerator.parse_client_id(client_id)
            return parsed["type"] in ["store", "agent"]
        except Exception:
            return False

    

def generate_id(length: int = 8) -> str:
    """
    生成随机ID
    
    Args:
        length: ID长度，默认8位
        
    Returns:
        str: 随机ID字符串
    """
    return ''.join(random.choices(string.ascii_lowercase + string.digits, k=length))


def generate_short_id(length: int = 4) -> str:
    """
    生成短随机ID
    
    Args:
        length: ID长度，默认4位
        
    Returns:
        str: 短随机ID字符串
    """
    return ''.join(random.choices(string.ascii_lowercase + string.digits, k=length))


def generate_uuid() -> str:
    """
    生成UUID
    
    Returns:
        str: UUID字符串
    """
    return str(uuid.uuid4())

