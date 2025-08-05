import json
import logging
import os
import random
import string
from datetime import datetime
from typing import Dict, Any, Optional, List

logger = logging.getLogger(__name__)

# Put all configuration files in the data/defaults directory
CLIENT_SERVICES_PATH = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'defaults', 'client_services.json')
AGENT_CLIENTS_PATH = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'defaults', 'agent_clients.json')

class ClientManager:
    """Class for managing client configurations"""
    
    def __init__(self, services_path: Optional[str] = None, agent_clients_path: Optional[str] = None, global_agent_store_id: Optional[str] = None):
        """
        Initialize client manager

        Args:
            services_path: Client service configuration file path
            agent_clients_path: Agent client mapping file path
            global_agent_store_id: Global agent store ID (optional, for data space)
        """
        self.services_path = services_path or CLIENT_SERVICES_PATH
        self.agent_clients_path = agent_clients_path or AGENT_CLIENTS_PATH
        self._ensure_file()
        self.client_services = self.load_all_clients()
        # 🔧 Fix: Support data space global_agent_store_id
        self.global_agent_store_id = global_agent_store_id or self._generate_data_space_client_id()
        self._ensure_agent_clients_file()

    def _generate_data_space_client_id(self) -> str:
        """
        Generate global_agent_store_id

        Returns:
            str: Fixed return "global_agent_store"
        """
        # Store-level Agent is fixed as global_agent_store
        return "global_agent_store"

    def _ensure_file(self):
        """Ensure client service configuration file exists"""
        os.makedirs(os.path.dirname(self.services_path), exist_ok=True)
        if not os.path.exists(self.services_path):
            with open(self.services_path, 'w', encoding='utf-8') as f:
                json.dump({}, f)

    def _ensure_agent_clients_file(self):
        """确保agent-client映射文件存在"""
        os.makedirs(os.path.dirname(self.agent_clients_path), exist_ok=True)
        if not os.path.exists(self.agent_clients_path):
            with open(self.agent_clients_path, 'w', encoding='utf-8') as f:
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
        with open(self.agent_clients_path, 'r', encoding='utf-8') as f:
            return json.load(f)

    def save_all_agent_clients(self, data: Dict[str, Any]):
        """保存agent-client映射"""
        with open(self.agent_clients_path, 'w', encoding='utf-8') as f:
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

    def get_all_agent_ids(self) -> List[str]:
        """🔧 [REFACTOR] 获取所有Agent ID列表 - 从文件读取"""
        agent_data = self.load_all_agent_clients()
        agent_ids = list(agent_data.keys())
        logger.debug(f"🔧 [CLIENT_MANAGER] Getting all agent IDs from file: {agent_ids}")
        return agent_ids

    def get_global_agent_store_ids(self) -> List[str]:
        """获取 global_agent_store 下的所有 client_id"""
        return list(self.get_all_clients().keys())

    def is_valid_client(self, client_id: str) -> bool:
        """检查是否是有效的 client_id"""
        return self.has_client(client_id)

    def find_clients_with_service(self, agent_id: str, service_name: str) -> List[str]:
        """
        查找指定Agent下包含特定服务的所有client_id

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            包含该服务的client_id列表
        """
        client_ids = self.get_agent_clients(agent_id)
        matching_clients = []

        for client_id in client_ids:
            client_config = self.get_client_config(client_id)
            if client_config and service_name in client_config.get("mcpServers", {}):
                matching_clients.append(client_id)

        return matching_clients

    def replace_service_in_agent(self, agent_id: str, service_name: str, new_service_config: Dict[str, Any]) -> bool:
        """
        在指定Agent中替换同名服务

        Store级别：删除所有包含该服务的client，创建新client
        Agent级别：只替换包含该服务的client

        Args:
            agent_id: Agent ID (global_agent_store for Store level)
            service_name: 服务名称
            new_service_config: 新的服务配置

        Returns:
            是否成功替换
        """
        try:
            # 1. 查找包含该服务的所有client_id
            matching_clients = self.find_clients_with_service(agent_id, service_name)

            if not matching_clients:
                # 没有找到同名服务，直接创建新的client
                logger.info(f"No existing service '{service_name}' found for agent {agent_id}, creating new client")
                return self._create_new_service_client(agent_id, service_name, new_service_config)

            # 2. Store级别：完全替换策略
            if agent_id == self.global_agent_store_id:
                logger.info(f"Store level: Replacing service '{service_name}' in {len(matching_clients)} clients")

                # 删除所有包含该服务的旧client
                for client_id in matching_clients:
                    self._remove_client_and_mapping(agent_id, client_id)
                    logger.info(f"Removed old client {client_id} containing service '{service_name}'")

                # 创建新的client
                return self._create_new_service_client(agent_id, service_name, new_service_config)

            # 3. Agent级别：精确替换策略
            else:
                logger.info(f"Agent level: Replacing service '{service_name}' in {len(matching_clients)} clients for agent {agent_id}")

                # 对每个包含该服务的client进行替换
                for client_id in matching_clients:
                    client_config = self.get_client_config(client_id)
                    if client_config:
                        # 更新服务配置
                        client_config["mcpServers"][service_name] = new_service_config
                        self.save_client_config_with_return(client_id, client_config)
                        logger.info(f"Updated service '{service_name}' in client {client_id}")

                return True

        except Exception as e:
            logger.error(f"Failed to replace service '{service_name}' for agent {agent_id}: {e}")
            return False

    def _create_new_service_client(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> bool:
        """
        为指定服务创建新的client

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置

        Returns:
            是否成功创建
        """
        try:
            # 生成新的client_id
            new_client_id = self.generate_client_id()

            # 创建client配置
            client_config = {
                "mcpServers": {
                    service_name: service_config
                }
            }

            # 保存client配置
            self.save_client_config_with_return(new_client_id, client_config)

            # 添加agent-client映射
            self.add_agent_client_mapping(agent_id, new_client_id)

            logger.info(f"Created new client {new_client_id} for service '{service_name}' under agent {agent_id}")
            return True

        except Exception as e:
            logger.error(f"Failed to create new client for service '{service_name}': {e}")
            return False

    def _remove_client_and_mapping(self, agent_id: str, client_id: str) -> bool:
        """
        删除client配置和agent映射

        Args:
            agent_id: Agent ID
            client_id: Client ID

        Returns:
            是否成功删除
        """
        try:
            # 删除client配置
            self.remove_client(client_id)

            # 删除agent-client映射
            self.remove_agent_client_mapping(agent_id, client_id)

            return True

        except Exception as e:
            logger.error(f"Failed to remove client {client_id} and mapping for agent {agent_id}: {e}")
            return False

    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> bool:
        """
        添加Agent-Client映射关系

        Args:
            agent_id: Agent ID
            client_id: Client ID

        Returns:
            是否成功添加
        """
        try:
            data = self.load_all_agent_clients()
            if agent_id not in data:
                data[agent_id] = []

            if client_id not in data[agent_id]:
                data[agent_id].append(client_id)
                self.save_all_agent_clients(data)
                logger.info(f"Added client {client_id} to agent {agent_id}")

            return True

        except Exception as e:
            logger.error(f"Failed to add agent-client mapping: {e}")
            return False

    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> bool:
        """
        移除Agent-Client映射关系

        Args:
            agent_id: Agent ID
            client_id: Client ID

        Returns:
            是否成功移除
        """
        try:
            data = self.load_all_agent_clients()
            if agent_id in data and client_id in data[agent_id]:
                data[agent_id].remove(client_id)

                # 如果Agent没有任何Client了，删除Agent条目
                if not data[agent_id]:
                    del data[agent_id]

                self.save_all_agent_clients(data)
                logger.info(f"Removed client {client_id} from agent {agent_id}")

            return True

        except Exception as e:
            logger.error(f"Failed to remove agent-client mapping: {e}")
            return False

    def save_client_config_with_return(self, client_id: str, config: Dict[str, Any]) -> bool:
        """
        保存Client配置（带返回值版本）

        Args:
            client_id: Client ID
            config: Client配置

        Returns:
            是否成功保存
        """
        try:
            # 使用已存在的方法
            self.save_client_config(client_id, config)
            return True

        except Exception as e:
            logger.error(f"Failed to save client config: {e}")
            return False

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

    # === 文件直接重置功能 ===
    def reset_client_services_file(self) -> bool:
        """
        直接重置client_services.json文件
        备份后重置为空字典

        Returns:
            是否成功重置
        """
        try:
            import shutil
            from datetime import datetime

            # 创建备份 - 统一使用.bak后缀
            backup_path = f"{self.services_path}.bak"
            if os.path.exists(self.services_path):
                shutil.copy2(self.services_path, backup_path)
                logger.info(f"Created backup of client_services.json at {backup_path}")

            # 重置为空配置
            empty_config = {}
            self.save_all_clients(empty_config)

            logger.info("Successfully reset client_services.json file")
            return True

        except Exception as e:
            logger.error(f"Failed to reset client_services.json file: {e}")
            return False

    def reset_agent_clients_file(self) -> bool:
        """
        直接重置agent_clients.json文件
        备份后重置为空字典

        Returns:
            是否成功重置
        """
        try:
            import shutil
            from datetime import datetime

            # 创建备份 - 统一使用.bak后缀
            backup_path = f"{self.agent_clients_path}.bak"
            if os.path.exists(self.agent_clients_path):
                shutil.copy2(self.agent_clients_path, backup_path)
                logger.info(f"Created backup of agent_clients.json at {backup_path}")

            # 重置为空配置
            empty_config = {}
            self.save_all_agent_clients(empty_config)

            logger.info("Successfully reset agent_clients.json file")
            return True

        except Exception as e:
            logger.error(f"Failed to reset agent_clients.json file: {e}")
            return False

    def remove_agent_from_files(self, agent_id: str) -> bool:
        """
        从文件中删除指定Agent的相关配置
        1. 从agent_clients.json中删除该agent的映射
        2. 从client_services.json中删除该agent关联的client配置

        Args:
            agent_id: 要删除的Agent ID

        Returns:
            是否成功删除
        """
        try:
            # 获取该Agent的所有client_id
            client_ids = self.get_agent_clients(agent_id)

            # 从client_services.json中删除相关client配置
            all_clients = self.load_all_clients()
            for client_id in client_ids:
                if client_id in all_clients:
                    del all_clients[client_id]
                    logger.info(f"Removed client {client_id} from client_services.json")
            self.save_all_clients(all_clients)

            # 从agent_clients.json中删除agent映射
            agent_data = self.load_all_agent_clients()
            if agent_id in agent_data:
                del agent_data[agent_id]
                self.save_all_agent_clients(agent_data)
                logger.info(f"Removed agent {agent_id} from agent_clients.json")

            logger.info(f"Successfully removed agent {agent_id} from all files")
            return True

        except Exception as e:
            logger.error(f"Failed to remove agent {agent_id} from files: {e}")
            return False

    def remove_store_from_files(self, global_agent_store_id: str) -> bool:
        """
        从文件中删除Store(global_agent_store)的相关配置
        1. 从client_services.json中删除global_agent_store的配置
        2. 从agent_clients.json中删除global_agent_store的映射

        Args:
            global_agent_store_id: Store的global_agent_store ID

        Returns:
            是否成功删除
        """
        try:
            # 从client_services.json中删除global_agent_store配置
            all_clients = self.load_all_clients()
            if global_agent_store_id in all_clients:
                del all_clients[global_agent_store_id]
                self.save_all_clients(all_clients)
                logger.info(f"Removed global_agent_store {global_agent_store_id} from client_services.json")

            # 从agent_clients.json中删除global_agent_store映射
            agent_data = self.load_all_agent_clients()
            if global_agent_store_id in agent_data:
                del agent_data[global_agent_store_id]
                self.save_all_agent_clients(agent_data)
                logger.info(f"Removed global_agent_store {global_agent_store_id} from agent_clients.json")

            logger.info(f"Successfully removed store global_agent_store {global_agent_store_id} from all files")
            return True

        except Exception as e:
            logger.error(f"Failed to remove store global_agent_store {global_agent_store_id} from files: {e}")
            return False


