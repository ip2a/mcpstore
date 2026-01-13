#!/usr/bin/env python3
"""
配置快照与调试可观测性模块

提供当前生效配置的快照导出能力，支持：
- 按组/键过滤配置项
- 显示配置值和来源（默认/TOML/KV/环境变量）
- 敏感数据屏蔽
- 多种输出格式（JSON/YAML/表格）
"""

import json
import sys
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Union

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "src"))

# 延迟导入配置默认值以避免依赖问题
def _get_config_defaults():
    try:
        from mcpstore.config.config_defaults import (
            HealthCheckConfigDefaults,
            ContentUpdateConfigDefaults,
            MonitoringConfigDefaults,
            CacheMemoryConfigDefaults,
            CacheRedisConfigDefaults,
            StandaloneConfigDefaults,
        )
        return {
            'HealthCheckConfigDefaults': HealthCheckConfigDefaults,
            'ContentUpdateConfigDefaults': ContentUpdateConfigDefaults,
            'MonitoringConfigDefaults': MonitoringConfigDefaults,
            'CacheMemoryConfigDefaults': CacheMemoryConfigDefaults,
            'CacheRedisConfigDefaults': CacheRedisConfigDefaults,
            'StandaloneConfigDefaults': StandaloneConfigDefaults,
        }
    except ImportError:
        # 如果导入失败，返回空的默认值
        return {}


class ConfigSource(Enum):
    """配置来源枚举"""
    DEFAULT = "default"         # 硬编码默认值
    TOML = "toml"              # TOML 文件
    KV = "kv"                  # KV 存储（运行时修改）
    ENV = "env"                # 环境变量
    COMPUTED = "computed"      # 计算得出的值


@dataclass
class ConfigItemSnapshot:
    """单个配置项的快照"""
    key: str                          # 配置键名（如 "health_check.failure_threshold"）
    value: Any                       # 配置值
    source: ConfigSource             # 配置来源
    category: str                    # 配置分类（如 "health_check", "cache"）
    is_sensitive: bool = False       # 是否为敏感配置
    is_dynamic: bool = False         # 是否为动态配置
    description: Optional[str] = None  # 配置描述
    validation_info: Optional[str] = None  # 验证信息（范围、枚举值等）


@dataclass
class ConfigGroupSnapshot:
    """配置组快照（如 health_check 组、cache 组）"""
    name: str                        # 组名
    items: List[ConfigItemSnapshot]  # 组内配置项
    item_count: int = field(init=False)  # 配置项数量

    def __post_init__(self):
        self.item_count = len(self.items)

    def get_item_count(self) -> int:
        """获取配置项数量"""
        return len(self.items)

    def get_sensitive_count(self) -> int:
        """获取敏感配置项数量"""
        return sum(1 for item in self.items if item.is_sensitive)

    def get_dynamic_count(self) -> int:
        """获取动态配置项数量"""
        return sum(1 for item in self.items if item.is_dynamic)


@dataclass
class ConfigSnapshot:
    """完整的配置快照"""
    timestamp: datetime                              # 快照时间戳
    groups: Dict[str, ConfigGroupSnapshot]         # 配置组字典
    total_items: int = field(init=False)           # 总配置项数
    source_summary: Dict[ConfigSource, int] = field(default_factory=dict)  # 来源统计

    def __post_init__(self):
        self.total_items = sum(group.item_count for group in self.groups.values())
        self._update_source_summary()

    def _update_source_summary(self):
        """更新来源统计"""
        self.source_summary.clear()
        for group in self.groups.values():
            for item in group.items:
                self.source_summary[item.source] = self.source_summary.get(item.source, 0) + 1

    def get_group(self, name: str) -> Optional[ConfigGroupSnapshot]:
        """获取指定配置组"""
        return self.groups.get(name)

    def get_all_keys(self) -> Set[str]:
        """获取所有配置键名"""
        return {item.key for group in self.groups.values() for item in group.items}

    def filter_by_category(self, categories: Union[str, List[str]]) -> 'ConfigSnapshot':
        """按分类过滤配置快照"""
        if isinstance(categories, str):
            categories = [categories]

        filtered_groups = {}
        for category in categories:
            if category in self.groups:
                filtered_groups[category] = self.groups[category]

        return ConfigSnapshot(
            timestamp=self.timestamp,
            groups=filtered_groups
        )

    def filter_by_key_pattern(self, pattern: str) -> 'ConfigSnapshot':
        """按键名模式过滤配置快照"""
        import re
        regex = re.compile(pattern, re.IGNORECASE)

        filtered_groups = {}
        for group_name, group in self.groups.items():
            filtered_items = [
                item for item in group.items
                if regex.search(item.key)
            ]
            if filtered_items:
                filtered_groups[group_name] = ConfigGroupSnapshot(
                    name=group_name,
                    items=filtered_items
                )

        return ConfigSnapshot(
            timestamp=self.timestamp,
            groups=filtered_groups
        )

    def to_dict(self, mask_sensitive: bool = True) -> Dict[str, Any]:
        """转换为字典格式"""
        result = {
            "timestamp": self.timestamp.isoformat(),
            "summary": {
                "total_items": self.total_items,
                "group_count": len(self.groups),
                "source_distribution": {source.value: count for source, count in self.source_summary.items()}
            },
            "groups": {}
        }

        for group_name, group in self.groups.items():
            group_dict = {
                "name": group.name,
                "item_count": group.item_count,
                "sensitive_count": group.get_sensitive_count(),
                "dynamic_count": group.get_dynamic_count(),
                "items": []
            }

            for item in group.items:
                item_dict = {
                    "key": item.key,
                    "value": "***MASKED***" if mask_sensitive and item.is_sensitive else item.value,
                    "source": item.source.value,
                    "category": item.category,
                    "is_sensitive": item.is_sensitive,
                    "is_dynamic": item.is_dynamic
                }
                if item.description:
                    item_dict["description"] = item.description
                if item.validation_info:
                    item_dict["validation_info"] = item.validation_info

                group_dict["items"].append(item_dict)

            result["groups"][group_name] = group_dict

        return result


class ConfigSnapshotFormatter:
    """配置快照格式化器"""

    @staticmethod
    def format_json(snapshot: ConfigSnapshot, mask_sensitive: bool = True, indent: int = 2) -> str:
        """格式化为 JSON"""
        return json.dumps(snapshot.to_dict(mask_sensitive), indent=indent, ensure_ascii=False)

    @staticmethod
    def format_yaml(snapshot: ConfigSnapshot, mask_sensitive: bool = True) -> str:
        """格式化为 YAML"""
        try:
            import yaml
            return yaml.dump(snapshot.to_dict(mask_sensitive), default_flow_style=False, allow_unicode=True)
        except ImportError:
            return "# PyYAML not installed, falling back to JSON\n" + \
                   ConfigSnapshotFormatter.format_json(snapshot, mask_sensitive)

    @staticmethod
    def format_table(snapshot: ConfigSnapshot, mask_sensitive: bool = True, max_width: int = 100) -> str:
        """格式化为表格"""
        lines = []
        lines.append("=" * max_width)
        lines.append(f"配置快照 - {snapshot.timestamp.strftime('%Y-%m-%d %H:%M:%S')}")
        lines.append("=" * max_width)
        lines.append(f"总计: {snapshot.total_items} 项配置，{len(snapshot.groups)} 个组")

        # 来源统计
        source_lines = [f"  {source.value}: {count}" for source, count in snapshot.source_summary.items()]
        lines.append("来源分布:\n" + "\n".join(source_lines))
        lines.append("")

        # 按组显示
        for group_name, group in snapshot.groups.items():
            lines.append(f"【{group_name}】({group.item_count} 项，{group.get_sensitive_count()} 敏感，{group.get_dynamic_count()} 动态)")
            lines.append("-" * max_width)

            for item in group.items:
                value_display = "***MASKED***" if mask_sensitive and item.is_sensitive else str(item.value)
                line = f"  {item.key:<35} = {value_display:<25} [{item.source.value}]"

                if item.is_sensitive:
                    line += " [SENSITIVE]"
                if item.is_dynamic:
                    line += " [DYNAMIC]"

                lines.append(line)

                if item.description:
                    lines.append(f"    └─ {item.description}")
                if item.validation_info:
                    lines.append(f"    └─ 验证: {item.validation_info}")

            lines.append("")

        return "\n".join(lines)


class ConfigSnapshotError(Exception):
    """配置快照相关异常"""
    pass
