"""
统一持久化管理器

核心设计原则：
1. Agent Client ID 映射到 Global Client ID
2. mcp.json 包含所有服务 (Store + Agent 服务，带后缀标识)
3. 统一的配置文件结构
4. 数据迁移和兼容性保证
"""

import json
import logging
import os
import uuid
from datetime import datetime
from typing import Dict, Any, Optional, List, Tuple
from pathlib import Path

logger = logging.getLogger(__name__)


class UnifiedPersistenceManager:
    """
    统一持久化管理器
    
    新架构特点：
    - 所有服务存储在 mcp.json 中 (包含 Agent 服务)
    - Agent Client ID 映射到 Global Client ID
    - 简化的文件结构
    - 向后兼容的数据迁移
    """
    
    def __init__(self, data_dir: str = None, mcp_json_path: str = None):
        """
        初始化统一持久化管理器
        
        Args:
            data_dir: 数据目录路径
            mcp_json_path: mcp.json 文件路径 (可选，用于指定特定文件)
        """
        # 确定数据目录
        if data_dir:
            self.data_dir = Path(data_dir)
        else:
            # 默认使用项目数据目录
            self.data_dir = Path(__file__).parent.parent.parent / "data"
        
        # 确定配置文件路径
        if mcp_json_path:
            self.mcp_json_path = Path(mcp_json_path)
            self.data_dir = self.mcp_json_path.parent
        else:
            self.mcp_json_path = self.data_dir / "mcp.json"
        
        # 其他配置文件路径
        self.agent_clients_path = self.data_dir / "agent_clients.json"
        self.client_services_path = self.data_dir / "client_services.json"
        
        # 确保目录和文件存在
        self._ensure_directory_structure()
        
        # 加载配置
        self.mcp_config = self._load_mcp_config()
        self.agent_clients = self._load_agent_clients()
        self.client_services = self._load_client_services()
        
        logger.info(f"🔄 [UNIFIED_PERSISTENCE] Initialized with data dir: {self.data_dir}")
    
    def _ensure_directory_structure(self):
        """确保目录结构存在"""
        try:
            # 创建数据目录
            self.data_dir.mkdir(parents=True, exist_ok=True)
            
            # 创建 mcp.json
            if not self.mcp_json_path.exists():
                default_mcp = {"mcpServers": {}}
                self._save_json(self.mcp_json_path, default_mcp)
                logger.info(f"📝 [UNIFIED_PERSISTENCE] Created default mcp.json")
            
            # 创建 agent_clients.json
            if not self.agent_clients_path.exists():
                default_agent_clients = {"global_agent_store": []}
                self._save_json(self.agent_clients_path, default_agent_clients)
                logger.info(f"📝 [UNIFIED_PERSISTENCE] Created default agent_clients.json")
            
            # 创建 client_services.json
            if not self.client_services_path.exists():
                default_client_services = {}
                self._save_json(self.client_services_path, default_client_services)
                logger.info(f"📝 [UNIFIED_PERSISTENCE] Created default client_services.json")
                
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to ensure directory structure: {e}")
            raise
    
    def _load_json(self, file_path: Path) -> Dict[str, Any]:
        """加载 JSON 文件"""
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                return json.load(f)
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to load {file_path}: {e}")
            return {}
    
    def _save_json(self, file_path: Path, data: Dict[str, Any]):
        """保存 JSON 文件"""
        try:
            with open(file_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to save {file_path}: {e}")
            raise
    
    def _load_mcp_config(self) -> Dict[str, Any]:
        """加载 mcp.json 配置"""
        config = self._load_json(self.mcp_json_path)
        if "mcpServers" not in config:
            config["mcpServers"] = {}
        return config
    
    def _load_agent_clients(self) -> Dict[str, List[str]]:
        """加载 agent_clients.json 配置"""
        clients = self._load_json(self.agent_clients_path)
        if "global_agent_store" not in clients:
            clients["global_agent_store"] = []
        return clients
    
    def _load_client_services(self) -> Dict[str, Dict[str, Any]]:
        """加载 client_services.json 配置"""
        return self._load_json(self.client_services_path)
    
    # === 服务配置管理 ===
    
    def add_service_to_mcp(self, service_name: str, config: Dict[str, Any]) -> bool:
        """
        添加服务到 mcp.json
        
        Args:
            service_name: 服务名称 (全局名称，可能包含 _byagent_ 后缀)
            config: 服务配置
            
        Returns:
            bool: 添加是否成功
        """
        try:
            self.mcp_config["mcpServers"][service_name] = config
            self._save_json(self.mcp_json_path, self.mcp_config)
            
            logger.info(f"✅ [UNIFIED_PERSISTENCE] Added service '{service_name}' to mcp.json")
            return True
            
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to add service '{service_name}': {e}")
            return False
    
    def update_service_in_mcp(self, service_name: str, config: Dict[str, Any]) -> bool:
        """
        更新 mcp.json 中的服务
        
        Args:
            service_name: 服务名称
            config: 新的服务配置
            
        Returns:
            bool: 更新是否成功
        """
        try:
            if service_name not in self.mcp_config["mcpServers"]:
                logger.warning(f"⚠️ [UNIFIED_PERSISTENCE] Service '{service_name}' not found, adding as new")
            
            self.mcp_config["mcpServers"][service_name] = config
            self._save_json(self.mcp_json_path, self.mcp_config)
            
            logger.info(f"✅ [UNIFIED_PERSISTENCE] Updated service '{service_name}' in mcp.json")
            return True
            
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to update service '{service_name}': {e}")
            return False
    
    def remove_service_from_mcp(self, service_name: str) -> bool:
        """
        从 mcp.json 移除服务
        
        Args:
            service_name: 服务名称
            
        Returns:
            bool: 移除是否成功
        """
        try:
            if service_name in self.mcp_config["mcpServers"]:
                del self.mcp_config["mcpServers"][service_name]
                self._save_json(self.mcp_json_path, self.mcp_config)
                
                logger.info(f"✅ [UNIFIED_PERSISTENCE] Removed service '{service_name}' from mcp.json")
                return True
            else:
                logger.warning(f"⚠️ [UNIFIED_PERSISTENCE] Service '{service_name}' not found in mcp.json")
                return True  # 已经不存在，视为成功
                
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to remove service '{service_name}': {e}")
            return False
    
    def get_service_from_mcp(self, service_name: str) -> Optional[Dict[str, Any]]:
        """
        从 mcp.json 获取服务配置
        
        Args:
            service_name: 服务名称
            
        Returns:
            服务配置或None
        """
        return self.mcp_config["mcpServers"].get(service_name)
    
    def get_all_services_from_mcp(self) -> Dict[str, Dict[str, Any]]:
        """获取 mcp.json 中的所有服务"""
        return self.mcp_config["mcpServers"].copy()
    
    def get_services_by_agent(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        """
        按 Agent 筛选服务
        
        Args:
            agent_id: Agent ID
            
        Returns:
            该 Agent 的服务配置
        """
        if agent_id == "global_agent_store":
            # Store 原生服务 (不包含 _byagent_ 的服务)
            return {
                name: config 
                for name, config in self.mcp_config["mcpServers"].items()
                if "_byagent_" not in name
            }
        else:
            # 特定 Agent 的服务
            agent_suffix = f"_byagent_{agent_id}"
            return {
                name: config 
                for name, config in self.mcp_config["mcpServers"].items()
                if name.endswith(agent_suffix)
            }
    
    # === Client 映射管理 ===
    
    def generate_client_id(self) -> str:
        """生成新的 Client ID"""
        return f"client_{uuid.uuid4().hex[:8]}_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    
    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> bool:
        """
        添加 Agent-Client 映射
        
        Args:
            agent_id: Agent ID
            client_id: Client ID
            
        Returns:
            bool: 添加是否成功
        """
        try:
            if agent_id not in self.agent_clients:
                self.agent_clients[agent_id] = []
            
            if client_id not in self.agent_clients[agent_id]:
                self.agent_clients[agent_id].append(client_id)
                self._save_json(self.agent_clients_path, self.agent_clients)
                
                logger.info(f"✅ [UNIFIED_PERSISTENCE] Added client mapping: {agent_id} -> {client_id}")
                return True
            else:
                logger.debug(f"🔄 [UNIFIED_PERSISTENCE] Client mapping already exists: {agent_id} -> {client_id}")
                return True
                
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to add client mapping: {e}")
            return False
    
    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> bool:
        """
        移除 Agent-Client 映射
        
        Args:
            agent_id: Agent ID
            client_id: Client ID
            
        Returns:
            bool: 移除是否成功
        """
        try:
            if agent_id in self.agent_clients and client_id in self.agent_clients[agent_id]:
                self.agent_clients[agent_id].remove(client_id)
                
                # 如果 Agent 没有 Client 了，移除 Agent 条目
                if not self.agent_clients[agent_id]:
                    del self.agent_clients[agent_id]
                
                self._save_json(self.agent_clients_path, self.agent_clients)
                
                logger.info(f"✅ [UNIFIED_PERSISTENCE] Removed client mapping: {agent_id} -> {client_id}")
                return True
            else:
                logger.warning(f"⚠️ [UNIFIED_PERSISTENCE] Client mapping not found: {agent_id} -> {client_id}")
                return True  # 已经不存在，视为成功
                
        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to remove client mapping: {e}")
            return False
    
    def get_agent_clients(self, agent_id: str) -> List[str]:
        """
        获取 Agent 的所有 Client ID
        
        Args:
            agent_id: Agent ID
            
        Returns:
            Client ID 列表
        """
        return self.agent_clients.get(agent_id, []).copy()
    
    def get_all_agent_clients(self) -> Dict[str, List[str]]:
        """获取所有 Agent-Client 映射"""
        return self.agent_clients.copy()

    # === Client 服务配置管理 ===

    def add_client_service_config(self, client_id: str, config: Dict[str, Any]) -> bool:
        """
        添加 Client 服务配置

        Args:
            client_id: Client ID
            config: Client 配置

        Returns:
            bool: 添加是否成功
        """
        try:
            self.client_services[client_id] = config
            self._save_json(self.client_services_path, self.client_services)

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Added client service config for {client_id}")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to add client service config: {e}")
            return False

    def update_client_service_config(self, client_id: str, config: Dict[str, Any]) -> bool:
        """
        更新 Client 服务配置

        Args:
            client_id: Client ID
            config: 新的 Client 配置

        Returns:
            bool: 更新是否成功
        """
        try:
            self.client_services[client_id] = config
            self._save_json(self.client_services_path, self.client_services)

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Updated client service config for {client_id}")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to update client service config: {e}")
            return False

    def remove_client_service_config(self, client_id: str) -> bool:
        """
        移除 Client 服务配置

        Args:
            client_id: Client ID

        Returns:
            bool: 移除是否成功
        """
        try:
            if client_id in self.client_services:
                del self.client_services[client_id]
                self._save_json(self.client_services_path, self.client_services)

                logger.info(f"✅ [UNIFIED_PERSISTENCE] Removed client service config for {client_id}")
                return True
            else:
                logger.warning(f"⚠️ [UNIFIED_PERSISTENCE] Client service config not found: {client_id}")
                return True  # 已经不存在，视为成功

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to remove client service config: {e}")
            return False

    def get_client_service_config(self, client_id: str) -> Optional[Dict[str, Any]]:
        """
        获取 Client 服务配置

        Args:
            client_id: Client ID

        Returns:
            Client 配置或None
        """
        return self.client_services.get(client_id)

    def get_all_client_service_configs(self) -> Dict[str, Dict[str, Any]]:
        """获取所有 Client 服务配置"""
        return self.client_services.copy()

    # === 数据迁移和兼容性 ===

    def migrate_from_legacy_format(self, legacy_client_services: Dict[str, Any],
                                  legacy_agent_clients: Dict[str, List[str]]) -> bool:
        """
        从旧格式迁移数据

        Args:
            legacy_client_services: 旧的 client_services 数据
            legacy_agent_clients: 旧的 agent_clients 数据

        Returns:
            bool: 迁移是否成功
        """
        try:
            logger.info("🔄 [UNIFIED_PERSISTENCE] Starting data migration from legacy format")

            # 迁移 client_services
            migrated_services = 0
            for client_id, client_config in legacy_client_services.items():
                if isinstance(client_config, dict) and "mcpServers" in client_config:
                    # 将 client 中的服务添加到 mcp.json
                    for service_name, service_config in client_config["mcpServers"].items():
                        if self.add_service_to_mcp(service_name, service_config):
                            migrated_services += 1

                    # 保留 client 配置
                    self.add_client_service_config(client_id, client_config)

            # 迁移 agent_clients
            migrated_mappings = 0
            for agent_id, client_ids in legacy_agent_clients.items():
                if isinstance(client_ids, list):
                    for client_id in client_ids:
                        if self.add_agent_client_mapping(agent_id, client_id):
                            migrated_mappings += 1

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Migration completed: {migrated_services} services, {migrated_mappings} mappings")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Migration failed: {e}")
            return False

    def backup_current_data(self) -> str:
        """
        备份当前数据

        Returns:
            str: 备份目录路径
        """
        try:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            backup_dir = self.data_dir / f"backup_{timestamp}"
            backup_dir.mkdir(exist_ok=True)

            # 备份所有配置文件
            files_to_backup = [
                (self.mcp_json_path, "mcp.json"),
                (self.agent_clients_path, "agent_clients.json"),
                (self.client_services_path, "client_services.json")
            ]

            for source_path, filename in files_to_backup:
                if source_path.exists():
                    backup_path = backup_dir / filename
                    with open(source_path, 'r', encoding='utf-8') as src:
                        with open(backup_path, 'w', encoding='utf-8') as dst:
                            dst.write(src.read())

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Data backed up to: {backup_dir}")
            return str(backup_dir)

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Backup failed: {e}")
            raise

    def restore_from_backup(self, backup_dir: str) -> bool:
        """
        从备份恢复数据

        Args:
            backup_dir: 备份目录路径

        Returns:
            bool: 恢复是否成功
        """
        try:
            backup_path = Path(backup_dir)
            if not backup_path.exists():
                logger.error(f"❌ [UNIFIED_PERSISTENCE] Backup directory not found: {backup_dir}")
                return False

            # 恢复所有配置文件
            files_to_restore = [
                ("mcp.json", self.mcp_json_path),
                ("agent_clients.json", self.agent_clients_path),
                ("client_services.json", self.client_services_path)
            ]

            for filename, target_path in files_to_restore:
                source_path = backup_path / filename
                if source_path.exists():
                    with open(source_path, 'r', encoding='utf-8') as src:
                        with open(target_path, 'w', encoding='utf-8') as dst:
                            dst.write(src.read())

            # 重新加载配置
            self.mcp_config = self._load_mcp_config()
            self.agent_clients = self._load_agent_clients()
            self.client_services = self._load_client_services()

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Data restored from: {backup_dir}")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Restore failed: {e}")
            return False

    # === 数据验证和修复 ===

    def validate_data_integrity(self) -> Tuple[bool, List[str]]:
        """
        验证数据完整性

        Returns:
            (is_valid, issues): 验证结果和问题列表
        """
        issues = []

        try:
            # 验证 mcp.json 结构
            if not isinstance(self.mcp_config, dict):
                issues.append("mcp.json is not a valid dictionary")
            elif "mcpServers" not in self.mcp_config:
                issues.append("mcp.json missing 'mcpServers' key")
            elif not isinstance(self.mcp_config["mcpServers"], dict):
                issues.append("mcp.json 'mcpServers' is not a dictionary")

            # 验证 agent_clients.json 结构
            if not isinstance(self.agent_clients, dict):
                issues.append("agent_clients.json is not a valid dictionary")
            else:
                for agent_id, client_ids in self.agent_clients.items():
                    if not isinstance(client_ids, list):
                        issues.append(f"agent_clients.json: {agent_id} should map to a list")

            # 验证 client_services.json 结构
            if not isinstance(self.client_services, dict):
                issues.append("client_services.json is not a valid dictionary")

            # 验证引用完整性
            all_client_ids = set()
            for client_ids in self.agent_clients.values():
                all_client_ids.update(client_ids)

            for client_id in all_client_ids:
                if client_id not in self.client_services:
                    issues.append(f"Client {client_id} referenced in agent_clients but not found in client_services")

            is_valid = len(issues) == 0

            if is_valid:
                logger.info("✅ [UNIFIED_PERSISTENCE] Data integrity validation passed")
            else:
                logger.warning(f"⚠️ [UNIFIED_PERSISTENCE] Data integrity issues found: {len(issues)}")
                for issue in issues:
                    logger.warning(f"  - {issue}")

            return is_valid, issues

        except Exception as e:
            issues.append(f"Validation error: {str(e)}")
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Validation failed: {e}")
            return False, issues

    def repair_data_integrity(self) -> bool:
        """
        修复数据完整性问题

        Returns:
            bool: 修复是否成功
        """
        try:
            logger.info("🔧 [UNIFIED_PERSISTENCE] Starting data integrity repair")

            # 修复 mcp.json 结构
            if not isinstance(self.mcp_config, dict):
                self.mcp_config = {}
            if "mcpServers" not in self.mcp_config:
                self.mcp_config["mcpServers"] = {}
            if not isinstance(self.mcp_config["mcpServers"], dict):
                self.mcp_config["mcpServers"] = {}

            # 修复 agent_clients.json 结构
            if not isinstance(self.agent_clients, dict):
                self.agent_clients = {"global_agent_store": []}

            for agent_id, client_ids in list(self.agent_clients.items()):
                if not isinstance(client_ids, list):
                    self.agent_clients[agent_id] = []

            # 确保 global_agent_store 存在
            if "global_agent_store" not in self.agent_clients:
                self.agent_clients["global_agent_store"] = []

            # 修复 client_services.json 结构
            if not isinstance(self.client_services, dict):
                self.client_services = {}

            # 修复引用完整性
            all_client_ids = set()
            for client_ids in self.agent_clients.values():
                all_client_ids.update(client_ids)

            for client_id in all_client_ids:
                if client_id not in self.client_services:
                    # 创建空的 client 配置
                    self.client_services[client_id] = {"mcpServers": {}}

            # 保存修复后的数据
            self._save_json(self.mcp_json_path, self.mcp_config)
            self._save_json(self.agent_clients_path, self.agent_clients)
            self._save_json(self.client_services_path, self.client_services)

            logger.info("✅ [UNIFIED_PERSISTENCE] Data integrity repair completed")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Data repair failed: {e}")
            return False

    # === 统计和监控 ===

    def get_storage_statistics(self) -> Dict[str, Any]:
        """获取存储统计信息"""
        try:
            stats = {
                "data_directory": str(self.data_dir),
                "mcp_json_path": str(self.mcp_json_path),
                "total_services": len(self.mcp_config.get("mcpServers", {})),
                "store_native_services": 0,
                "agent_services": 0,
                "agents_with_services": [],
                "total_clients": len(self.client_services),
                "total_agent_client_mappings": sum(len(clients) for clients in self.agent_clients.values()),
                "file_sizes": {}
            }

            # 分析服务类型
            for service_name in self.mcp_config.get("mcpServers", {}):
                if "_byagent_" in service_name:
                    stats["agent_services"] += 1
                    # 提取 agent_id
                    parts = service_name.split("_byagent_")
                    if len(parts) == 2:
                        agent_id = parts[1]
                        if agent_id not in stats["agents_with_services"]:
                            stats["agents_with_services"].append(agent_id)
                else:
                    stats["store_native_services"] += 1

            # 获取文件大小
            for file_path in [self.mcp_json_path, self.agent_clients_path, self.client_services_path]:
                if file_path.exists():
                    stats["file_sizes"][file_path.name] = file_path.stat().st_size

            return stats

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to get storage statistics: {e}")
            return {}

    # === 补充 ClientManager 的遗漏功能 ===

    def create_client_config_from_names(self, service_names: List[str]) -> Dict[str, Any]:
        """
        从服务名称列表生成客户端配置

        Args:
            service_names: 服务名称列表

        Returns:
            客户端配置
        """
        all_services = self.get_all_services_from_mcp()
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

        self.add_client_service_config(client_id, config)
        return client_id

    def remove_client(self, client_id: str) -> bool:
        """
        移除客户端配置

        Args:
            client_id: 要移除的客户端ID

        Returns:
            bool: 移除是否成功
        """
        return self.remove_client_service_config(client_id)

    def has_client(self, client_id: str) -> bool:
        """
        检查客户端是否存在

        Args:
            client_id: 客户端ID

        Returns:
            bool: 客户端是否存在
        """
        return client_id in self.get_all_client_service_configs()

    def is_valid_client(self, client_id: str) -> bool:
        """
        检查是否是有效的客户端ID

        Args:
            client_id: 客户端ID

        Returns:
            bool: 是否有效
        """
        return self.has_client(client_id)

    def find_clients_with_service(self, agent_id: str, service_name: str) -> List[str]:
        """
        查找包含指定服务的客户端

        Args:
            agent_id: Agent ID
            service_name: 服务名称 (本地名称)

        Returns:
            包含该服务的客户端ID列表
        """
        # 转换为全局服务名称
        if agent_id != "global_agent_store" and "_byagent_" not in service_name:
            from mcpstore.core.agent_service_mapper import AgentServiceMapper
            mapper = AgentServiceMapper(agent_id)
            global_service_name = mapper.to_global_name(service_name)
        else:
            global_service_name = service_name

        matching_clients = []

        # 获取该 Agent 的所有客户端
        agent_clients = self.get_agent_clients(agent_id)

        for client_id in agent_clients:
            client_config = self.get_client_service_config(client_id)
            if client_config and "mcpServers" in client_config:
                if global_service_name in client_config["mcpServers"]:
                    matching_clients.append(client_id)

        return matching_clients

    def replace_service_in_agent(self, agent_id: str, service_name: str, new_service_config: Dict[str, Any]) -> bool:
        """
        替换 Agent 中的服务配置

        Args:
            agent_id: Agent ID
            service_name: 服务名称 (本地名称)
            new_service_config: 新的服务配置

        Returns:
            bool: 替换是否成功
        """
        try:
            # 转换为全局服务名称
            if agent_id != "global_agent_store" and "_byagent_" not in service_name:
                from mcpstore.core.agent_service_mapper import AgentServiceMapper
                mapper = AgentServiceMapper(agent_id)
                global_service_name = mapper.to_global_name(service_name)
            else:
                global_service_name = service_name

            # 更新 mcp.json 中的服务配置
            success = self.update_service_in_mcp(global_service_name, new_service_config)

            if success:
                # 更新所有包含该服务的客户端配置
                matching_clients = self.find_clients_with_service(agent_id, service_name)

                for client_id in matching_clients:
                    client_config = self.get_client_service_config(client_id)
                    if client_config and "mcpServers" in client_config:
                        client_config["mcpServers"][global_service_name] = new_service_config
                        self.update_client_service_config(client_id, client_config)

                logger.info(f"✅ [UNIFIED_PERSISTENCE] Replaced service '{service_name}' in agent '{agent_id}' and {len(matching_clients)} clients")
                return True
            else:
                return False

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to replace service '{service_name}' in agent '{agent_id}': {e}")
            return False

    def reset_agent_config(self, agent_id: str) -> bool:
        """
        重置 Agent 配置

        Args:
            agent_id: Agent ID

        Returns:
            bool: 重置是否成功
        """
        try:
            # 获取该 Agent 的所有客户端
            agent_clients = self.get_agent_clients(agent_id)

            # 移除所有客户端配置
            for client_id in agent_clients:
                self.remove_client_service_config(client_id)

            # 清空 Agent-Client 映射
            self.agent_clients[agent_id] = []
            self._save_json(self.agent_clients_path, self.agent_clients)

            # 移除该 Agent 的所有服务
            if agent_id != "global_agent_store":
                agent_services = self.get_services_by_agent(agent_id)
                for service_name in list(agent_services.keys()):
                    self.remove_service_from_mcp(service_name)

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Reset agent config for '{agent_id}'")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to reset agent config for '{agent_id}': {e}")
            return False

    def remove_agent_from_files(self, agent_id: str) -> bool:
        """
        从文件中完全移除 Agent

        Args:
            agent_id: Agent ID

        Returns:
            bool: 移除是否成功
        """
        try:
            # 重置 Agent 配置 (这会清理服务和客户端)
            self.reset_agent_config(agent_id)

            # 从 agent_clients.json 中移除 Agent 条目
            if agent_id in self.agent_clients:
                del self.agent_clients[agent_id]
                self._save_json(self.agent_clients_path, self.agent_clients)

            logger.info(f"✅ [UNIFIED_PERSISTENCE] Removed agent '{agent_id}' from files")
            return True

        except Exception as e:
            logger.error(f"❌ [UNIFIED_PERSISTENCE] Failed to remove agent '{agent_id}' from files: {e}")
            return False
