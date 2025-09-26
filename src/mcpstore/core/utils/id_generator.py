"""
Client ID Generator Module
Provides unified and deterministic client ID generation for MCPStore
"""

import hashlib
import logging
import uuid
import random
import string
from typing import Dict, Any

logger = logging.getLogger(__name__)


class ClientIDGenerator:
    """
    统一的Client ID生成器

    提供确定性的client_id生成算法，确保：
    1. 相同的输入总是产生相同的ID
    2. 不同的Agent/Service组合产生不同的ID
    3. 支持Store和Agent两种模式
    """

    @staticmethod
    def generate_deterministic_id(agent_id: str, service_name: str,
                                  service_config: Dict[str, Any],
                                  global_agent_store_id: str) -> str:
        """
        生成确定性的client_id

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置（用于生成hash）
            global_agent_store_id: 全局Agent Store ID

        Returns:
            str: 确定性的client_id

        格式说明：
        - Store服务: client_store_{service_name}_{config_hash}
        - Agent服务: client_{agent_id}_{service_name}_{config_hash}
        """
        try:
            # 生成配置哈希（确保确定性）
            config_str = str(sorted(service_config.items())) if service_config else ""
            config_hash = hashlib.md5(config_str.encode()).hexdigest()[:8]

            # 根据agent类型生成不同格式的client_id
            if agent_id == global_agent_store_id:
                # Store服务格式
                client_id = f"client_store_{service_name}_{config_hash}"
                logger.debug(f" [ID_GEN] Generated Store client_id: {service_name} -> {client_id}")
            else:
                # Agent服务格式
                client_id = f"client_{agent_id}_{service_name}_{config_hash}"
                logger.debug(f" [ID_GEN] Generated Agent client_id: {agent_id}:{service_name} -> {client_id}")

            return client_id

        except Exception as e:
            logger.error(f"❌ [ID_GEN] Failed to generate client_id for {agent_id}:{service_name}: {e}")
            # 回退到简单格式
            fallback_id = f"client_{agent_id}_{service_name}_fallback"
            logger.warning(f"⚠️ [ID_GEN] Using fallback client_id: {fallback_id}")
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
            logger.error(f"❌ [ID_GEN] Error parsing client_id {client_id}: {e}")
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

    @staticmethod
    def migrate_legacy_id(legacy_id: str, agent_id: str, service_name: str,
                         service_config: Dict[str, Any],
                         global_agent_store_id: str) -> str:
        """
        将旧格式的client_id迁移到新的确定性格式

        Args:
            legacy_id: 旧的client_id
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置
            global_agent_store_id: 全局Agent Store ID

        Returns:
            str: 新的确定性client_id
        """
        logger.info(f" [ID_GEN] Migrating legacy client_id: {legacy_id} -> deterministic format")

        new_id = ClientIDGenerator.generate_deterministic_id(
            agent_id, service_name, service_config, global_agent_store_id
        )

        logger.info(f"✅ [ID_GEN] Migration completed: {legacy_id} -> {new_id}")
        return new_id


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

