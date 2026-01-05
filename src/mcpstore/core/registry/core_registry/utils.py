"""
Utils - 工具函数模块

包含从原 core_registry.py 中提取的工具函数，包括：
1. JSON Schema 解析相关函数
2. 数据类型推断函数
3. 配置处理工具函数
4. 其他辅助函数
"""

import json
import logging
from typing import Dict, Any, List, Optional, Tuple

logger = logging.getLogger(__name__)


class JSONSchemaUtils:
    """JSON Schema 处理工具类"""

    @staticmethod
    def extract_description_from_schema(prop_info: Dict[str, Any]) -> str:
        """
        从JSON Schema属性信息中提取描述

        Args:
            prop_info: JSON Schema属性信息

        Returns:
            描述字符串
        """
        if not isinstance(prop_info, dict):
            return "参数"

        # 优先使用 description 字段
        description = prop_info.get('description')
        if description:
            return description

        # 尝试从其他字段推断
        if prop_info.get('type'):
            type_desc = prop_info['type']
            if isinstance(type_desc, list):
                type_desc = " 或 ".join(type_desc)
            return f"{type_desc} 类型参数"

        # 检查 enum 值
        if 'enum' in prop_info:
            enum_values = prop_info['enum']
            if enum_values:
                values_str = ", ".join(str(v) for v in enum_values[:3])
                if len(enum_values) > 3:
                    values_str += "..."
                return f"可选值: {values_str}"

        return "参数"

    @staticmethod
    def extract_type_from_schema(prop_info: Dict[str, Any]) -> str:
        """
        从JSON Schema属性信息中提取类型

        Args:
            prop_info: JSON Schema属性信息

        Returns:
            类型字符串
        """
        if not isinstance(prop_info, dict):
            return "any"

        # 获取类型信息
        prop_type = prop_info.get('type')

        if prop_type is None:
            # 检查其他类型指示字段
            if 'enum' in prop_info:
                return "enum"
            elif 'const' in prop_info:
                return "const"
            elif 'anyOf' in prop_info:
                return "anyOf"
            elif 'oneOf' in prop_info:
                return "oneOf"
            elif 'allOf' in prop_info:
                return "allOf"
            else:
                return "any"

        # 处理类型是列表的情况（联合类型）
        if isinstance(prop_type, list):
            # 过滤掉 null
            non_null_types = [t for t in prop_type if t != 'null']
            if not non_null_types:
                return "null"
            elif len(non_null_types) == 1:
                return non_null_types[0]
            else:
                return f"({' | '.join(non_null_types)})"

        return str(prop_type)

    @staticmethod
    def get_default_value_from_schema(prop_info: Dict[str, Any]) -> Any:
        """
        从JSON Schema属性信息中获取默认值

        Args:
            prop_info: JSON Schema属性信息

        Returns:
            默认值或None
        """
        if not isinstance(prop_info, dict):
            return None

        # 优先使用 default 字段
        if 'default' in prop_info:
            return prop_info['default']

        # 对于布尔类型，默认为 False
        if prop_info.get('type') == 'boolean':
            return False

        # 对于数组类型，默认为空数组
        if prop_info.get('type') == 'array':
            return []

        # 对于对象类型，默认为空对象
        if prop_info.get('type') == 'object':
            return {}

        # 对于字符串类型，默认为空字符串
        if prop_info.get('type') == 'string':
            return ""

        # 对于数字类型，默认为 0
        if prop_info.get('type') in ['number', 'integer']:
            return 0

        return None

    @staticmethod
    def is_required_parameter(prop_name: str, required_params: List[str]) -> bool:
        """
        检查参数是否为必需参数

        Args:
            prop_name: 参数名称
            required_params: 必需参数列表

        Returns:
            是否为必需参数
        """
        return prop_name in required_params

    @staticmethod
    def format_parameter_info(param_name: str, prop_info: Dict[str, Any],
                            required_params: List[str]) -> Dict[str, Any]:
        """
        格式化参数信息

        Args:
            param_name: 参数名称
            prop_info: JSON Schema属性信息
            required_params: 必需参数列表

        Returns:
            格式化的参数信息
        """
        return {
            'name': param_name,
            'type': JSONSchemaUtils.extract_type_from_schema(prop_info),
            'description': JSONSchemaUtils.extract_description_from_schema(prop_info),
            'default': JSONSchemaUtils.get_default_value_from_schema(prop_info),
            'required': JSONSchemaUtils.is_required_parameter(param_name, required_params),
            'enum': prop_info.get('enum') if 'enum' in prop_info else None
        }


class ConfigUtils:
    """配置处理工具类"""

    @staticmethod
    def validate_service_config_structure(config: Dict[str, Any]) -> Dict[str, Any]:
        """
        验证服务配置结构

        Args:
            config: 服务配置

        Returns:
            验证结果
        """
        result = {
            'valid': True,
            'errors': [],
            'warnings': []
        }

        # 检查基本结构
        if not isinstance(config, dict):
            result['valid'] = False
            result['errors'].append("配置必须是一个字典")
            return result

        # 检查是否为空配置
        if not config:
            result['valid'] = False
            result['errors'].append("配置不能为空")
            return result

        # 检查必要的连接信息
        has_command = 'command' in config
        has_url = 'url' in config

        if not (has_command or has_url):
            result['valid'] = False
            result['errors'].append("必须包含 'command' 或 'url' 字段")

        # 验证命令配置
        if has_command:
            command = config.get('command')
            if not isinstance(command, str) or not command.strip():
                result['valid'] = False
                result['errors'].append("'command' 必须是非空字符串")

            # 验证参数
            args = config.get('args', [])
            if args is not None and not isinstance(args, list):
                result['errors'].append("'args' 必须是数组类型")
                result['warnings'].append("将 'args' 重置为空数组")

        # 验证URL配置
        if has_url:
            url = config.get('url')
            if not isinstance(url, str) or not url.strip():
                result['valid'] = False
                result['errors'].append("'url' 必须是非空字符串")
            elif not (url.startswith('http://') or url.startswith('https://')):
                result['warnings'].append("URL 建议以 http:// 或 https:// 开头")

        # 验证环境变量
        if 'env' in config:
            env = config['env']
            if env is not None and not isinstance(env, dict):
                result['errors'].append("'env' 必须是字典类型")
                result['warnings'].append("将 'env' 重置为空字典")

        return result

    @staticmethod
    def normalize_service_config(config: Dict[str, Any]) -> Dict[str, Any]:
        """
        标准化服务配置

        Args:
            config: 原始配置

        Returns:
            标准化后的配置
        """
        normalized = config.copy()

        # 确保 args 是列表
        if 'args' in normalized and normalized['args'] is None:
            normalized['args'] = []
        elif 'args' not in normalized:
            normalized['args'] = []

        # 确保 env 是字典
        if 'env' in normalized and normalized['env'] is None:
            normalized['env'] = {}
        elif 'env' not in normalized:
            normalized['env'] = {}

        # 设置默认的传输类型
        if 'transport_type' not in normalized:
            if 'command' in normalized:
                normalized['transport_type'] = 'stdio'
            elif 'url' in normalized:
                normalized['transport_type'] = 'http'

        return normalized

    @staticmethod
    def merge_service_configs(base_config: Dict[str, Any],
                            override_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        合并服务配置

        Args:
            base_config: 基础配置
            override_config: 覆盖配置

        Returns:
            合并后的配置
        """
        merged = base_config.copy()

        for key, value in override_config.items():
            if key in ['args', 'env']:
                # 对于数组和字典类型的字段，进行合并而不是覆盖
                if isinstance(value, dict):
                    merged.setdefault(key, {}).update(value)
                elif isinstance(value, list):
                    merged.setdefault(key, []).extend(value)
            else:
                # 其他字段直接覆盖
                merged[key] = value

        return merged


class ServiceUtils:
    """服务相关工具类"""

    @staticmethod
    def generate_service_id(agent_id: str, service_name: str) -> str:
        """
        生成服务ID

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务ID
        """
        return f"{agent_id}::{service_name}"

    @staticmethod
    def parse_service_id(service_id: str) -> Tuple[str, str]:
        """
        解析服务ID

        Args:
            service_id: 服务ID

        Returns:
            (agent_id, service_name) 元组
        """
        if '::' not in service_id:
            raise ValueError(f"Invalid service ID format: {service_id}")

        parts = service_id.split('::', 1)
        if len(parts) != 2:
            raise ValueError(f"Invalid service ID format: {service_id}")

        return parts[0], parts[1]

    @staticmethod
    def is_valid_service_name(service_name: str) -> bool:
        """
        验证服务名称是否有效

        Args:
            service_name: 服务名称

        Returns:
            是否有效
        """
        if not service_name or not isinstance(service_name, str):
            return False

        # 服务名称不能包含特殊字符
        invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|']
        if any(char in service_name for char in invalid_chars):
            return False

        # 长度限制
        if len(service_name) > 100:
            return False

        return True

    @staticmethod
    def sanitize_service_name(service_name: str) -> str:
        """
        清理服务名称，移除无效字符

        Args:
            service_name: 原始服务名称

        Returns:
            清理后的服务名称
        """
        if not service_name:
            return "unnamed_service"

        # 移除无效字符
        invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|']
        sanitized = service_name
        for char in invalid_chars:
            sanitized = sanitized.replace(char, '_')

        # 长度限制
        if len(sanitized) > 100:
            sanitized = sanitized[:97] + '...'

        # 确保不为空
        if not sanitized.strip():
            sanitized = "unnamed_service"

        return sanitized.strip()


class DataUtils:
    """数据处理工具类"""

    @staticmethod
    def deep_merge_dict(base: Dict[str, Any], override: Dict[str, Any]) -> Dict[str, Any]:
        """
        深度合并字典

        Args:
            base: 基础字典
            override: 覆盖字典

        Returns:
            合并后的字典
        """
        result = base.copy()

        for key, value in override.items():
            if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                result[key] = DataUtils.deep_merge_dict(result[key], value)
            else:
                result[key] = value

        return result

    @staticmethod
    def safe_json_serialize(obj: Any) -> Optional[str]:
        """
        安全的JSON序列化

        Args:
            obj: 要序列化的对象

        Returns:
            JSON字符串或None
        """
        try:
            return json.dumps(obj, default=str, ensure_ascii=False)
        except Exception as e:
            logger.warning(f"JSON serialization failed: {e}")
            return None

    @staticmethod
    def safe_json_deserialize(json_str: str) -> Optional[Any]:
        """
        安全的JSON反序列化

        Args:
            json_str: JSON字符串

        Returns:
            反序列化的对象或None
        """
        try:
            return json.loads(json_str)
        except Exception as e:
            logger.warning(f"JSON deserialization failed: {e}")
            return None

    @staticmethod
    def flatten_dict(d: Dict[str, Any], parent_key: str = '', sep: str = '.') -> Dict[str, Any]:
        """
        扁平化字典

        Args:
            d: 要扁平化的字典
            parent_key: 父键名
            sep: 分隔符

        Returns:
            扁平化后的字典
        """
        items = []
        for k, v in d.items():
            new_key = f"{parent_key}{sep}{k}" if parent_key else k
            if isinstance(v, dict):
                items.extend(DataUtils.flatten_dict(v, new_key, sep=sep).items())
            else:
                items.append((new_key, v))
        return dict(items)


class ValidationUtils:
    """验证工具类"""

    @staticmethod
    def validate_agent_id(agent_id: str) -> bool:
        """
        验证Agent ID是否有效

        Args:
            agent_id: Agent ID

        Returns:
            是否有效
        """
        if not agent_id or not isinstance(agent_id, str):
            return False

        # 基本长度检查
        if len(agent_id) < 1 or len(agent_id) > 100:
            return False

        # 字符检查：只允许字母、数字、下划线、连字符
        import re
        pattern = r'^[a-zA-Z0-9_-]+$'
        return bool(re.match(pattern, agent_id))

    @staticmethod
    def validate_global_name(global_name: str) -> bool:
        """
        验证全局名称是否有效

        Args:
            global_name: 全局名称

        Returns:
            是否有效
        """
        if not global_name or not isinstance(global_name, str):
            return False

        # 基本格式检查：应该包含agent_id和服务名
        if '::' not in global_name:
            return False

        try:
            parts = global_name.split('::', 1)
            agent_id, service_name = parts
            return (ValidationUtils.validate_agent_id(agent_id) and
                   ServiceUtils.is_valid_service_name(service_name))
        except Exception:
            return False

    @staticmethod
    def sanitize_agent_id(agent_id: str) -> str:
        """
        清理Agent ID

        Args:
            agent_id: 原始Agent ID

        Returns:
            清理后的Agent ID
        """
        if not agent_id:
            return "unknown_agent"

        # 移除无效字符，只保留字母、数字、下划线、连字符
        import re
        sanitized = re.sub(r'[^a-zA-Z0-9_-]', '_', agent_id)

        # 长度限制
        if len(sanitized) > 100:
            sanitized = sanitized[:97] + '...'

        # 确保不为空
        if not sanitized.strip():
            sanitized = "unknown_agent"

        return sanitized.strip()


# 便捷函数，保持向后兼容
def extract_description_from_schema(prop_info: Dict[str, Any]) -> str:
    """向后兼容的函数"""
    return JSONSchemaUtils.extract_description_from_schema(prop_info)


def extract_type_from_schema(prop_info: Dict[str, Any]) -> str:
    """向后兼容的函数"""
    return JSONSchemaUtils.extract_type_from_schema(prop_info)
