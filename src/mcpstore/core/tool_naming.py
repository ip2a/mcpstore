#!/usr/bin/env python3
"""
工具命名管理器 - 解决服务名与工具名拼接的健壮性问题
提供安全的工具命名和解析机制
"""

import re
import hashlib
from typing import Tuple, Optional, List
import logging

logger = logging.getLogger(__name__)

class ToolNamingManager:
    """
    工具命名管理器
    
    解决服务名包含下划线时的命名冲突问题，提供健壮的工具命名机制
    
    设计原则：
    1. 使用特殊分隔符避免冲突
    2. 支持服务名包含下划线
    3. 提供双向转换（编码/解码）
    4. 保持向后兼容性
    """
    
    # 使用双下划线作为分隔符，降低冲突概率
    SEPARATOR = "__"
    
    # 服务名和工具名的最大长度限制
    MAX_SERVICE_NAME_LENGTH = 50
    MAX_TOOL_NAME_LENGTH = 100
    MAX_FULL_NAME_LENGTH = 200
    
    # 有效字符正则表达式
    VALID_NAME_PATTERN = re.compile(r'^[a-zA-Z0-9_\-\.]+$')
    
    @classmethod
    def create_tool_name(cls, service_name: str, original_tool_name: str) -> str:
        """
        创建安全的工具名称
        
        Args:
            service_name: 服务名称
            original_tool_name: 原始工具名称
            
        Returns:
            安全的完整工具名称
            
        Raises:
            ValueError: 如果名称不符合规范
        """
        # 验证输入
        cls._validate_name(service_name, "service_name")
        cls._validate_name(original_tool_name, "tool_name")
        
        # 清理名称（移除不安全字符）
        clean_service_name = cls._clean_name(service_name)
        clean_tool_name = cls._clean_name(original_tool_name)
        
        # 检查长度限制
        if len(clean_service_name) > cls.MAX_SERVICE_NAME_LENGTH:
            clean_service_name = cls._truncate_with_hash(clean_service_name, cls.MAX_SERVICE_NAME_LENGTH)
            
        if len(clean_tool_name) > cls.MAX_TOOL_NAME_LENGTH:
            clean_tool_name = cls._truncate_with_hash(clean_tool_name, cls.MAX_TOOL_NAME_LENGTH)
        
        # 创建完整工具名
        full_name = f"{clean_service_name}{cls.SEPARATOR}{clean_tool_name}"
        
        # 检查总长度
        if len(full_name) > cls.MAX_FULL_NAME_LENGTH:
            # 如果太长，使用哈希压缩
            full_name = cls._create_compressed_name(clean_service_name, clean_tool_name)
        
        logger.debug(f"Created tool name: {service_name}::{original_tool_name} -> {full_name}")
        return full_name
    
    @classmethod
    def parse_tool_name(cls, full_tool_name: str) -> Tuple[Optional[str], str]:
        """
        解析完整工具名称，提取服务名和原始工具名
        
        Args:
            full_tool_name: 完整的工具名称
            
        Returns:
            (service_name, original_tool_name) 元组
            如果无法解析服务名，则service_name为None
        """
        if not full_tool_name:
            return None, ""
        
        # 检查是否包含分隔符
        if cls.SEPARATOR in full_tool_name:
            parts = full_tool_name.split(cls.SEPARATOR, 1)  # 只分割第一个分隔符
            if len(parts) == 2:
                service_name, tool_name = parts
                logger.debug(f"Parsed tool name: {full_tool_name} -> {service_name}::{tool_name}")
                return service_name, tool_name
        
        # 尝试兼容旧的单下划线格式
        if "_" in full_tool_name:
            # 尝试从已知服务列表中匹配
            # 这需要传入已知服务列表，暂时返回None
            logger.debug(f"Could not parse service from tool name: {full_tool_name}")
            return None, full_tool_name
        
        # 没有分隔符，认为是纯工具名
        return None, full_tool_name
    
    @classmethod
    def belongs_to_service(cls, full_tool_name: str, service_name: str) -> bool:
        """
        判断工具是否属于指定服务
        
        Args:
            full_tool_name: 完整工具名称
            service_name: 服务名称
            
        Returns:
            是否属于该服务
        """
        parsed_service, _ = cls.parse_tool_name(full_tool_name)
        
        if parsed_service:
            return parsed_service == service_name
        
        # 兼容旧格式：检查是否以"服务名_"开头
        clean_service_name = cls._clean_name(service_name)
        return full_tool_name.startswith(f"{clean_service_name}_")
    
    @classmethod
    def get_tools_for_service(cls, all_tool_names: List[str], service_name: str) -> List[str]:
        """
        从工具名列表中筛选属于指定服务的工具
        
        Args:
            all_tool_names: 所有工具名列表
            service_name: 服务名称
            
        Returns:
            属于该服务的工具名列表
        """
        service_tools = []
        clean_service_name = cls._clean_name(service_name)
        
        for tool_name in all_tool_names:
            if cls.belongs_to_service(tool_name, service_name):
                service_tools.append(tool_name)
        
        logger.debug(f"Found {len(service_tools)} tools for service '{service_name}': {service_tools}")
        return service_tools
    
    @classmethod
    def migrate_old_tool_name(cls, old_tool_name: str, service_name: str) -> str:
        """
        将旧格式的工具名迁移到新格式
        
        Args:
            old_tool_name: 旧格式工具名（可能是service_tool格式）
            service_name: 服务名称
            
        Returns:
            新格式的工具名
        """
        clean_service_name = cls._clean_name(service_name)
        
        # 如果已经是新格式，直接返回
        if cls.SEPARATOR in old_tool_name:
            return old_tool_name
        
        # 如果是旧格式，尝试提取原始工具名
        if old_tool_name.startswith(f"{clean_service_name}_"):
            original_tool_name = old_tool_name[len(clean_service_name) + 1:]
            return cls.create_tool_name(service_name, original_tool_name)
        
        # 如果不匹配，可能是纯工具名，直接创建新格式
        return cls.create_tool_name(service_name, old_tool_name)
    
    @classmethod
    def _validate_name(cls, name: str, name_type: str) -> None:
        """验证名称是否符合规范"""
        if not name:
            raise ValueError(f"{name_type} cannot be empty")
        
        if not isinstance(name, str):
            raise ValueError(f"{name_type} must be a string")
        
        # 检查是否包含双下划线（保留分隔符）
        if cls.SEPARATOR in name:
            raise ValueError(f"{name_type} cannot contain '{cls.SEPARATOR}' (reserved separator)")
        
        # 检查基本字符规范
        if not cls.VALID_NAME_PATTERN.match(name):
            logger.warning(f"{name_type} '{name}' contains invalid characters, will be cleaned")
    
    @classmethod
    def _clean_name(cls, name: str) -> str:
        """清理名称，移除不安全字符"""
        # 只保留字母、数字、下划线、连字符、点号
        cleaned = re.sub(r'[^a-zA-Z0-9_\-\.]', '_', name)
        
        # 移除连续的下划线
        cleaned = re.sub(r'_+', '_', cleaned)
        
        # 移除开头和结尾的下划线
        cleaned = cleaned.strip('_')
        
        # 确保不为空
        if not cleaned:
            cleaned = "unnamed"
        
        return cleaned
    
    @classmethod
    def _truncate_with_hash(cls, name: str, max_length: int) -> str:
        """截断名称并添加哈希后缀以保证唯一性"""
        if len(name) <= max_length:
            return name
        
        # 计算哈希
        hash_suffix = hashlib.md5(name.encode()).hexdigest()[:8]
        
        # 截断并添加哈希
        truncated_length = max_length - len(hash_suffix) - 1  # -1 for underscore
        truncated = name[:truncated_length]
        
        return f"{truncated}_{hash_suffix}"
    
    @classmethod
    def _create_compressed_name(cls, service_name: str, tool_name: str) -> str:
        """创建压缩的工具名称"""
        # 为整个名称创建哈希
        full_name = f"{service_name}{cls.SEPARATOR}{tool_name}"
        name_hash = hashlib.md5(full_name.encode()).hexdigest()[:16]
        
        # 保留部分原始名称以便识别
        max_service_len = 20
        max_tool_len = 20
        
        short_service = service_name[:max_service_len]
        short_tool = tool_name[:max_tool_len]
        
        return f"{short_service}{cls.SEPARATOR}{short_tool}_{name_hash}"
    
    @classmethod
    def get_separator(cls) -> str:
        """获取当前使用的分隔符"""
        return cls.SEPARATOR
    
    @classmethod
    def is_new_format(cls, tool_name: str) -> bool:
        """判断是否为新格式的工具名"""
        return cls.SEPARATOR in tool_name
    
    @classmethod
    def get_original_tool_name(cls, full_tool_name: str) -> str:
        """获取原始工具名（去除服务前缀）"""
        _, original_name = cls.parse_tool_name(full_tool_name)
        return original_name
