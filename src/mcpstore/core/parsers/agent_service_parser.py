"""
Agent 服务解析器

统一的 Agent 服务名解析和验证逻辑，支持：
1. Agent 服务名格式验证
2. Agent ID 和本地服务名提取
3. 全局服务名生成
4. 批量解析和验证

设计原则:
1. 统一的解析逻辑
2. 严格的格式验证
3. 详细的错误信息
4. 高性能批量处理
"""

import logging
import re
from typing import Dict, List, Tuple, Optional, Set, Any
from dataclasses import dataclass

logger = logging.getLogger(__name__)

@dataclass
class AgentServiceInfo:
    """Agent 服务信息"""
    agent_id: str
    local_name: str
    global_name: str
    is_valid: bool
    error_message: Optional[str] = None

class AgentServiceParser:
    """Agent 服务解析器"""
    
    # Agent 服务名格式：service_byagent_agentid
    AGENT_SERVICE_PATTERN = re.compile(r'^(.+)_byagent_([a-zA-Z0-9_-]+)$')
    AGENT_ID_PATTERN = re.compile(r'^[a-zA-Z0-9_-]+$')
    SERVICE_NAME_PATTERN = re.compile(r'^[a-zA-Z0-9_-]+$')
    
    def __init__(self):
        """初始化解析器"""
        self._cache: Dict[str, AgentServiceInfo] = {}
        
    def parse_agent_service_name(self, global_name: str) -> AgentServiceInfo:
        """
        解析 Agent 服务名
        
        Args:
            global_name: 全局服务名（格式: service_byagent_agentid）
            
        Returns:
            AgentServiceInfo: 解析结果
        """
        # 检查缓存
        if global_name in self._cache:
            return self._cache[global_name]
        
        try:
            # 基本格式验证
            if not global_name or not isinstance(global_name, str):
                result = AgentServiceInfo(
                    agent_id="",
                    local_name="",
                    global_name=global_name,
                    is_valid=False,
                    error_message="服务名不能为空或非字符串"
                )
                self._cache[global_name] = result
                return result
            
            # 正则匹配
            match = self.AGENT_SERVICE_PATTERN.match(global_name)
            if not match:
                result = AgentServiceInfo(
                    agent_id="",
                    local_name="",
                    global_name=global_name,
                    is_valid=False,
                    error_message=f"不符合 Agent 服务名格式: {global_name}"
                )
                self._cache[global_name] = result
                return result
            
            local_name, agent_id = match.groups()
            
            # 验证组件
            validation_error = self._validate_components(local_name, agent_id)
            if validation_error:
                result = AgentServiceInfo(
                    agent_id=agent_id,
                    local_name=local_name,
                    global_name=global_name,
                    is_valid=False,
                    error_message=validation_error
                )
                self._cache[global_name] = result
                return result
            
            # 成功解析
            result = AgentServiceInfo(
                agent_id=agent_id,
                local_name=local_name,
                global_name=global_name,
                is_valid=True
            )
            self._cache[global_name] = result
            return result
            
        except Exception as e:
            logger.error(f"❌ [PARSER] 解析 Agent 服务名失败 {global_name}: {e}")
            result = AgentServiceInfo(
                agent_id="",
                local_name="",
                global_name=global_name,
                is_valid=False,
                error_message=f"解析异常: {e}"
            )
            self._cache[global_name] = result
            return result
    
    def generate_global_name(self, agent_id: str, local_name: str) -> str:
        """
        生成全局服务名
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名
            
        Returns:
            str: 全局服务名
            
        Raises:
            ValueError: 如果参数无效
        """
        # 验证参数
        validation_error = self._validate_components(local_name, agent_id)
        if validation_error:
            raise ValueError(validation_error)
        
        return f"{local_name}_byagent_{agent_id}"
    
    def is_agent_service(self, service_name: str) -> bool:
        """
        判断是否为 Agent 服务
        
        Args:
            service_name: 服务名
            
        Returns:
            bool: 是否为 Agent 服务
        """
        if not service_name or not isinstance(service_name, str):
            return False
        
        return bool(self.AGENT_SERVICE_PATTERN.match(service_name))
    
    def extract_agent_id(self, global_name: str) -> Optional[str]:
        """
        提取 Agent ID
        
        Args:
            global_name: 全局服务名
            
        Returns:
            Optional[str]: Agent ID，如果不是 Agent 服务则返回 None
        """
        info = self.parse_agent_service_name(global_name)
        return info.agent_id if info.is_valid else None
    
    def extract_local_name(self, global_name: str) -> Optional[str]:
        """
        提取本地服务名
        
        Args:
            global_name: 全局服务名
            
        Returns:
            Optional[str]: 本地服务名，如果不是 Agent 服务则返回 None
        """
        info = self.parse_agent_service_name(global_name)
        return info.local_name if info.is_valid else None
    
    def batch_parse(self, service_names: List[str]) -> Dict[str, AgentServiceInfo]:
        """
        批量解析服务名
        
        Args:
            service_names: 服务名列表
            
        Returns:
            Dict[str, AgentServiceInfo]: 解析结果字典
        """
        results = {}
        for service_name in service_names:
            results[service_name] = self.parse_agent_service_name(service_name)
        return results
    
    def filter_agent_services(self, service_names: List[str]) -> List[str]:
        """
        筛选出 Agent 服务
        
        Args:
            service_names: 服务名列表
            
        Returns:
            List[str]: Agent 服务名列表
        """
        return [name for name in service_names if self.is_agent_service(name)]
    
    def group_by_agent(self, service_names: List[str]) -> Dict[str, List[str]]:
        """
        按 Agent 分组服务
        
        Args:
            service_names: 服务名列表
            
        Returns:
            Dict[str, List[str]]: Agent ID -> 全局服务名列表
        """
        groups = {}
        for service_name in service_names:
            info = self.parse_agent_service_name(service_name)
            if info.is_valid:
                if info.agent_id not in groups:
                    groups[info.agent_id] = []
                groups[info.agent_id].append(service_name)
        return groups
    
    def validate_service_name_format(self, service_name: str) -> Tuple[bool, Optional[str]]:
        """
        验证服务名格式
        
        Args:
            service_name: 服务名
            
        Returns:
            Tuple[bool, Optional[str]]: (是否有效, 错误信息)
        """
        if not service_name or not isinstance(service_name, str):
            return False, "服务名不能为空或非字符串"
        
        if not self.SERVICE_NAME_PATTERN.match(service_name):
            return False, f"服务名格式无效: {service_name}，只允许字母、数字、下划线和连字符"
        
        if len(service_name) > 100:
            return False, f"服务名过长: {len(service_name)} > 100"
        
        return True, None
    
    def validate_agent_id_format(self, agent_id: str) -> Tuple[bool, Optional[str]]:
        """
        验证 Agent ID 格式
        
        Args:
            agent_id: Agent ID
            
        Returns:
            Tuple[bool, Optional[str]]: (是否有效, 错误信息)
        """
        if not agent_id or not isinstance(agent_id, str):
            return False, "Agent ID 不能为空或非字符串"
        
        if not self.AGENT_ID_PATTERN.match(agent_id):
            return False, f"Agent ID 格式无效: {agent_id}，只允许字母、数字、下划线和连字符"
        
        if len(agent_id) > 50:
            return False, f"Agent ID 过长: {len(agent_id)} > 50"
        
        return True, None
    
    def get_cache_stats(self) -> Dict[str, Any]:
        """
        获取缓存统计信息
        
        Returns:
            Dict[str, Any]: 缓存统计信息
        """
        valid_count = sum(1 for info in self._cache.values() if info.is_valid)
        invalid_count = len(self._cache) - valid_count
        
        return {
            "total_cached": len(self._cache),
            "valid_count": valid_count,
            "invalid_count": invalid_count,
            "cache_hit_ratio": len(self._cache) / max(1, len(self._cache))
        }
    
    def clear_cache(self):
        """清空缓存"""
        self._cache.clear()
        logger.debug("🔧 [PARSER] 缓存已清空")
    
    def _validate_components(self, local_name: str, agent_id: str) -> Optional[str]:
        """
        验证服务名组件
        
        Args:
            local_name: 本地服务名
            agent_id: Agent ID
            
        Returns:
            Optional[str]: 错误信息，如果验证通过则返回 None
        """
        # 验证本地服务名
        is_valid, error = self.validate_service_name_format(local_name)
        if not is_valid:
            return f"本地服务名无效: {error}"
        
        # 验证 Agent ID
        is_valid, error = self.validate_agent_id_format(agent_id)
        if not is_valid:
            return f"Agent ID 无效: {error}"
        
        return None
