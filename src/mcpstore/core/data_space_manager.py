"""
Data Space Manager
Responsible for initializing and maintaining store data directories, ensuring each store has independent data space
"""

import json
import logging
import shutil
from datetime import datetime
from pathlib import Path
from typing import Dict, Any

from .registry.schema_manager import get_schema_manager

logger = logging.getLogger(__name__)

class DataSpaceManager:
    """Data Space Manager - responsible for initializing and maintaining store data directories"""

    # Required file definitions - maintain hierarchical structure consistent with default configuration
    REQUIRED_FILES = {
        "defaults/agent_clients.json": {
            "schema_name": "agent_clients",
            "description": "Agent client mapping file"
        },
        "defaults/client_services.json": {
            "schema_name": "client_services",
            "description": "Client service mapping file"
        }
    }
    
    def __init__(self, mcp_json_path: str):
        """
        Initialize data space manager

        Args:
            mcp_json_path: MCP JSON configuration file path
        """
        self.mcp_json_path = Path(mcp_json_path).resolve()
        self.workspace_dir = self.mcp_json_path.parent
        self.schema_manager = get_schema_manager()

        logger.info(f"DataSpaceManager initialized for workspace: {self.workspace_dir}")
    
    def initialize_workspace(self) -> bool:
        """
        Initialize workspace, ensure all required files exist and are valid

        Returns:
            bool: Whether initialization was successful
        """
        try:
            logger.info(f"Initializing workspace: {self.workspace_dir}")
            
            # 1. 确保工作空间目录存在
            self.workspace_dir.mkdir(parents=True, exist_ok=True)
            
            # 2. 检查和处理MCP JSON文件
            if not self._validate_and_fix_mcp_json():
                logger.error("Failed to validate/fix MCP JSON file")
                return False
            
            # 3. 检查和创建必需文件
            if not self._ensure_required_files():
                logger.error("Failed to ensure required files")
                return False
            
            logger.info("Workspace initialization completed successfully")
            return True
            
        except Exception as e:
            logger.error(f"Workspace initialization failed: {e}")
            return False
    
    def _validate_and_fix_mcp_json(self) -> bool:
        """
        验证和修复MCP JSON文件
        
        Returns:
            bool: 处理是否成功
        """
        try:
            if not self.mcp_json_path.exists():
                logger.info(f"MCP JSON file not found, creating: {self.mcp_json_path}")
                return self._create_mcp_json()
            
            # 尝试读取和验证现有文件
            try:
                with open(self.mcp_json_path, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                
                # 验证文件结构
                if self._validate_mcp_json_structure(data):
                    logger.info("MCP JSON file is valid")
                    return True
                else:
                    logger.warning("MCP JSON file structure is invalid, will backup and recreate")
                    return self._backup_and_recreate_mcp_json()
                    
            except json.JSONDecodeError as e:
                logger.warning(f"MCP JSON file has syntax errors: {e}, will backup and recreate")
                return self._backup_and_recreate_mcp_json()
            except Exception as e:
                logger.warning(f"Error reading MCP JSON file: {e}, will backup and recreate")
                return self._backup_and_recreate_mcp_json()
                
        except Exception as e:
            logger.error(f"Failed to validate/fix MCP JSON: {e}")
            return False
    
    def _validate_mcp_json_structure(self, data: Dict[str, Any]) -> bool:
        """
        Validate MCP JSON file structure

        Args:
            data: JSON data

        Returns:
            bool: Whether structure is valid
        """
        # Check required fields
        if not isinstance(data, dict):
            return False
        
        # Check mcpServers field
        if "mcpServers" not in data:
            return False
        
        if not isinstance(data["mcpServers"], dict):
            return False
        
        # Basic structure is valid
        return True
    
    def _backup_and_recreate_mcp_json(self) -> bool:
        """
        Backup existing file and recreate MCP JSON file

        Returns:
            bool: Whether operation was successful
        """
        try:
            # Create backup - uniformly use .bak suffix
            backup_path = Path(str(self.mcp_json_path) + '.bak')

            if self.mcp_json_path.exists():
                shutil.copy2(self.mcp_json_path, backup_path)
                logger.info(f"Backup created: {backup_path}")

            # Recreate file
            return self._create_mcp_json()

        except Exception as e:
            logger.error(f"Failed to backup and recreate MCP JSON: {e}")
            return False
    
    def _create_mcp_json(self) -> bool:
        """
        Create new MCP JSON file

        Returns:
            bool: Whether creation was successful
        """
        try:
            # Use Schema manager to get template
            template = self.schema_manager.get_mcp_config_template()

            with open(self.mcp_json_path, 'w', encoding='utf-8') as f:
                json.dump(template, f, indent=2, ensure_ascii=False)

            logger.info(f"Created new MCP JSON file: {self.mcp_json_path}")
            return True

        except Exception as e:
            logger.error(f"Failed to create MCP JSON file: {e}")
            return False
    
    def _ensure_required_files(self) -> bool:
        """
        Ensure all required files exist and are valid

        Returns:
            bool: Whether operation was successful
        """
        try:
            for file_path, config in self.REQUIRED_FILES.items():
                full_path = self.workspace_dir / file_path
                
                # Ensure directory exists
                full_path.parent.mkdir(parents=True, exist_ok=True)
                
                if not full_path.exists():
                    # File doesn't exist, create new file
                    self._create_file_from_template(full_path, config)
                    logger.info(f"Created missing file: {full_path}")
                else:
                    # File exists, validate format
                    if not self._validate_json_file(full_path):
                        # File format error, backup and recreate
                        self._backup_and_recreate_file(full_path, config)
                        logger.warning(f"Recreated invalid file: {full_path}")
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to ensure required files: {e}")
            return False
    
    def _create_file_from_template(self, file_path: Path, config: Dict[str, Any]) -> bool:
        """
        从模板创建文件

        Args:
            file_path: 文件路径
            config: 文件配置

        Returns:
            bool: 创建是否成功
        """
        try:
            # 使用Schema管理器获取模板
            schema_name = config.get("schema_name", "")
            template = self.schema_manager.get_template(schema_name)

            with open(file_path, 'w', encoding='utf-8') as f:
                json.dump(template, f, indent=2, ensure_ascii=False)
            return True
        except Exception as e:
            logger.error(f"Failed to create file {file_path}: {e}")
            return False
    
    def _validate_json_file(self, file_path: Path) -> bool:
        """
        验证JSON文件格式
        
        Args:
            file_path: 文件路径
            
        Returns:
            bool: 文件是否有效
        """
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                json.load(f)
            return True
        except (json.JSONDecodeError, Exception):
            return False
    
    def _backup_and_recreate_file(self, file_path: Path, config: Dict[str, Any]) -> bool:
        """
        备份并重新创建文件

        Args:
            file_path: 文件路径
            config: 文件配置

        Returns:
            bool: 操作是否成功
        """
        try:
            # 创建备份 - 统一使用.bak后缀
            backup_path = Path(str(file_path) + '.bak')

            if file_path.exists():
                shutil.copy2(file_path, backup_path)
                logger.info(f"Backup created: {backup_path}")

            # 重新创建文件
            return self._create_file_from_template(file_path, config)

        except Exception as e:
            logger.error(f"Failed to backup and recreate file {file_path}: {e}")
            return False
    
    def get_file_path(self, file_type: str) -> Path:
        """
        获取特定类型文件的路径
        
        Args:
            file_type: 文件类型 (如: 'agent_clients.json', 'monitoring/alerts.json')
            
        Returns:
            Path: 文件路径
        """
        if file_type == "mcp.json":
            return self.mcp_json_path
        
        if file_type in self.REQUIRED_FILES:
            return self.workspace_dir / file_type
        
        # 对于其他文件，直接返回相对于workspace的路径
        return self.workspace_dir / file_type
    
    def get_workspace_info(self) -> Dict[str, Any]:
        """
        获取工作空间信息
        
        Returns:
            Dict: 工作空间信息
        """
        info = {
            "workspace_dir": str(self.workspace_dir),
            "mcp_json_path": str(self.mcp_json_path),
            "files": {}
        }
        
        # 检查MCP JSON文件
        info["files"]["mcp.json"] = {
            "exists": self.mcp_json_path.exists(),
            "path": str(self.mcp_json_path)
        }
        
        # 检查必需文件
        for file_type in self.REQUIRED_FILES:
            file_path = self.get_file_path(file_type)
            info["files"][file_type] = {
                "exists": file_path.exists(),
                "path": str(file_path),
                "description": self.REQUIRED_FILES[file_type]["description"]
            }
        
        return info
