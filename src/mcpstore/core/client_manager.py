import os
import json
import random
import string
from datetime import datetime
from typing import Dict, Any, Optional, List
import logging

logger = logging.getLogger(__name__)

# 将所有配置文件统一放在 data/defaults 目录下
CLIENT_SERVICES_PATH = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'defaults', 'client_services.json')
AGENT_CLIENTS_PATH = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'defaults', 'agent_clients.json')

class ClientManager:
    """管理客户端配置的类"""
    
    def __init__(self, services_path: Optional[str] = None):
        """
        初始化客户端管理器
        
        Args:
            services_path: 配置文件目录
        """
        self.services_path = services_path or CLIENT_SERVICES_PATH
        self._ensure_file()
        self.client_services = self.load_all_clients()
        self.main_client_id = "main_client"  # 主客户端ID
        self._ensure_agent_clients_file()

    def _ensure_file(self):
        """确保客户端服务配置文件存在"""
        os.makedirs(os.path.dirname(self.services_path), exist_ok=True)
        if not os.path.exists(self.services_path):
            with open(self.services_path, 'w', encoding='utf-8') as f:
                json.dump({}, f)

    def _ensure_agent_clients_file(self):
        """确保agent-client映射文件存在"""
        os.makedirs(os.path.dirname(AGENT_CLIENTS_PATH), exist_ok=True)
        if not os.path.exists(AGENT_CLIENTS_PATH):
            with open(AGENT_CLIENTS_PATH, 'w', encoding='utf-8') as f:
                json.dump({}, f)

    def load_all_clients(self) -> Dict[str, Any]:
        """加载所有客户端配置"""
        with open(self.services_path, 'r', encoding='utf-8') as f:
            return json.load(f)

    def save_all_clients(self, data: Dict[str, Any]):
        """保存所有客户端配置"""
        with open(self.services_path, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        # 更新内存中的数据
        self.client_services = data.copy()

    def get_client_config(self, client_id: str) -> Optional[Dict[str, Any]]:
        """获取客户端配置"""
        # 每次都重新加载以确保数据最新
        self.client_services = self.load_all_clients()
        return self.client_services.get(client_id)

    def save_client_config(self, client_id: str, config: Dict[str, Any]):
        """保存客户端配置"""
        all_clients = self.load_all_clients()
        all_clients[client_id] = config
        self.save_all_clients(all_clients)
        logger.info(f"Saved config for client_id={client_id}")

    def generate_client_id(self) -> str:
        """生成唯一的客户端ID"""
        ts = datetime.now().strftime("%Y%m%d%H%M%S")
        rand = ''.join(random.choices(string.ascii_lowercase + string.digits, k=6))
        return f"client_{ts}_{rand}"

    def create_client_config_from_names(self, service_names: List[str], mcp_config: Dict[str, Any]) -> Dict[str, Any]:
        """从服务名称列表生成新的客户端配置"""
        all_services = mcp_config.get("mcpServers", {})
        selected = {name: all_services[name] for name in service_names if name in all_services}
        return {"mcpServers": selected}

    def add_client(self, config: Dict[str, Any], client_id: Optional[str] = None) -> str:
        """
        添加新的客户端配置
        
        Args:
            config: 客户端配置
            client_id: 可选的客户端ID，如果不提供则自动生成
            
        Returns:
            使用的客户端ID
        """
        if not client_id:
            client_id = self.generate_client_id()
        self.client_services[client_id] = config
        self.save_client_config(client_id, config)
        return client_id
    
    def remove_client(self, client_id: str) -> bool:
        """
        移除客户端配置
        
        Args:
            client_id: 要移除的客户端ID
            
        Returns:
            是否成功移除
        """
        if client_id in self.client_services:
            del self.client_services[client_id]
            self.save_all_clients(self.client_services)
            return True
        return False
    
    def has_client(self, client_id: str) -> bool:
        """
        检查客户端是否存在
        
        Args:
            client_id: 客户端ID
            
        Returns:
            是否存在
        """
        # 每次检查都重新加载以确保数据最新
        self.client_services = self.load_all_clients()
        return client_id in self.client_services
    
    def get_all_clients(self) -> Dict[str, Any]:
        """
        获取所有客户端配置
        
        Returns:
            所有客户端配置的字典
        """
        # 每次获取都重新加载以确保数据最新
        self.client_services = self.load_all_clients()
        return self.client_services.copy()

    # === agent_clients.json 相关 ===
    def load_all_agent_clients(self) -> Dict[str, Any]:
        """加载所有agent-client映射"""
        self._ensure_agent_clients_file()
        with open(AGENT_CLIENTS_PATH, 'r', encoding='utf-8') as f:
            return json.load(f)

    def save_all_agent_clients(self, data: Dict[str, Any]):
        """保存agent-client映射"""
        with open(AGENT_CLIENTS_PATH, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    def get_agent_clients(self, agent_id: str) -> List[str]:
        """
        获取指定 agent 下的所有 client_id
        """
        data = self.load_all_agent_clients()
        return data.get(agent_id, [])

    def add_agent_client_mapping(self, agent_id: str, client_id: str):
        """添加agent-client映射"""
        data = self.load_all_agent_clients()
        if agent_id not in data:
            data[agent_id] = [client_id]
        elif client_id not in data[agent_id]:
            data[agent_id].append(client_id)
        self.save_all_agent_clients(data)
        logger.info(f"Mapped agent_id={agent_id} to client_id={client_id}")

    def remove_agent_client_mapping(self, agent_id: str, client_id: str):
        """移除agent-client映射"""
        data = self.load_all_agent_clients()
        if agent_id in data and client_id in data[agent_id]:
            data[agent_id].remove(client_id)
            if not data[agent_id]:
                del data[agent_id]
            self.save_all_agent_clients(data)
            logger.info(f"Removed mapping agent_id={agent_id} to client_id={client_id}")

    def get_main_client_ids(self) -> List[str]:
        """获取 main_client 下的所有 client_id"""
        return list(self.get_all_clients().keys())

    def is_valid_client(self, client_id: str) -> bool:
        """检查是否是有效的 client_id"""
        return self.has_client(client_id)

    def reset_agent_config(self, agent_id: str) -> bool:
        """
        重置指定Agent的配置
        1. 删除该Agent的所有client配置
        2. 删除agent-client映射

        Args:
            agent_id: 要重置的Agent ID

        Returns:
            是否成功重置
        """
        try:
            # 获取该Agent的所有client_id
            client_ids = self.get_agent_clients(agent_id)

            # 删除所有client配置
            for client_id in client_ids:
                self.remove_client(client_id)
                logger.info(f"Removed client {client_id} for agent {agent_id}")

            # 删除agent-client映射
            data = self.load_all_agent_clients()
            if agent_id in data:
                del data[agent_id]
                self.save_all_agent_clients(data)
                logger.info(f"Removed agent-client mapping for agent {agent_id}")

            logger.info(f"Successfully reset config for agent {agent_id}")
            return True

        except Exception as e:
            logger.error(f"Failed to reset config for agent {agent_id}: {e}")
            return False


