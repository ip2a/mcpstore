#!/usr/bin/env python3
"""
配置导出服务

提供配置快照的导出功能，支持多种格式和输出方式
"""

import logging
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "src"))

from mcpstore.config.toml_config import get_config
from mcpstore.core.configuration.config_snapshot import (
    ConfigSnapshotFormatter, ConfigSnapshotError
)
from mcpstore.core.configuration.config_snapshot_generator import ConfigSnapshotGenerator

logger = logging.getLogger(__name__)


class ConfigExportService:
    """配置导出服务"""

    def __init__(self):
        """初始化配置导出服务"""
        self.generator = None
        self._init_generator()

    def _init_generator(self):
        """初始化快照生成器"""
        try:
            config = get_config()
            self.generator = ConfigSnapshotGenerator(config)
        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to initialize configuration snapshot generator: {e}")
            raise ConfigSnapshotError(f"Failed to initialize configuration export service: {e}")

    async def export_config(self,
                           format: str = "table",
                           categories: Optional[List[str]] = None,
                           key_pattern: Optional[str] = None,
                           include_sensitive: bool = False,
                           output_file: Optional[Union[str, Path]] = None,
                           mask_sensitive: bool = True) -> str:
        """
        导出配置快照

        Args:
            format: 输出格式 ("json", "yaml", "table")
            categories: 要包含的配置分类列表
            key_pattern: 键名过滤模式（正则表达式）
            include_sensitive: 是否包含敏感配置
            output_file: 输出文件路径，None 表示返回字符串
            mask_sensitive: 是否屏蔽敏感配置值

        Returns:
            str: 配置快照内容（如果 output_file 为 None）

        Raises:
            ConfigSnapshotError: 导出过程中的错误
        """
        if not self.generator:
            raise ConfigSnapshotError("Configuration snapshot generator not initialized")

        # 验证格式
        if format not in ["json", "yaml", "table"]:
            raise ConfigSnapshotError(f"Unsupported format: {format}, supported formats: json, yaml, table")

        try:
            # 生成配置快照
            snapshot = await self.generator.generate_snapshot(
                categories=categories,
                key_pattern=key_pattern,
                include_sensitive=include_sensitive
            )

            # 格式化输出
            if format == "json":
                content = ConfigSnapshotFormatter.format_json(snapshot, mask_sensitive=mask_sensitive)
            elif format == "yaml":
                content = ConfigSnapshotFormatter.format_yaml(snapshot, mask_sensitive=mask_sensitive)
            else:  # table
                content = ConfigSnapshotFormatter.format_table(
                    snapshot, mask_sensitive=mask_sensitive, max_width=120
                )

            # 输出到文件或返回字符串
            if output_file:
                await self._write_to_file(content, output_file)
                return f"Configuration exported to: {output_file}"
            else:
                return content

        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to export configuration: {e}")
            raise ConfigSnapshotError(f"Failed to export configuration: {e}")

    async def _write_to_file(self, content: str, file_path: Union[str, Path]):
        """写入内容到文件"""
        try:
            path = Path(file_path)
            path.parent.mkdir(parents=True, exist_ok=True)

            with open(path, 'w', encoding='utf-8') as f:
                f.write(content)

            logger.info(f"[CONFIG_EXPORT] [SAVE] Configuration snapshot saved to: {path}")
        except Exception as e:
            raise ConfigSnapshotError(f"Failed to write file {file_path}: {e}")

    async def get_config_summary(self) -> Dict[str, Any]:
        """
        获取配置摘要信息

        Returns:
            Dict[str, Any]: 配置摘要
        """
        if not self.generator:
            raise ConfigSnapshotError("Configuration snapshot generator not initialized")

        try:
            # 生成快照（不包含敏感配置）
            snapshot = await self.generator.generate_snapshot(include_sensitive=False)

            summary = {
                "timestamp": snapshot.timestamp.isoformat(),
                "total_items": snapshot.total_items,
                "group_count": len(snapshot.groups),
                "source_distribution": {
                    source.value: count for source, count in snapshot.source_summary.items()
                },
                "groups": {}
            }

            # 各组详细信息
            for group_name, group in snapshot.groups.items():
                summary["groups"][group_name] = {
                    "name": group.name,
                    "item_count": group.item_count,
                    "sensitive_count": group.get_sensitive_count(),
                    "dynamic_count": group.get_dynamic_count()
                }

            return summary

        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to get configuration summary: {e}")
            raise ConfigSnapshotError(f"Failed to get configuration summary: {e}")

    async def search_config(self,
                           query: str,
                           include_sensitive: bool = False) -> Dict[str, Any]:
        """
        搜索配置项

        Args:
            query: 搜索查询（键名或描述）
            include_sensitive: 是否包含敏感配置

        Returns:
            Dict[str, Any]: 搜索结果
        """
        if not self.generator:
            raise ConfigSnapshotError("Configuration snapshot generator not initialized")

        try:
            # 使用查询作为正则表达式过滤
            snapshot = await self.generator.generate_snapshot(
                key_pattern=query,
                include_sensitive=include_sensitive
            )

            results = []
            for group_name, group in snapshot.groups.items():
                for item in group.items:
                    results.append({
                        "key": item.key,
                        "value": item.value,
                        "source": item.source.value,
                        "category": item.category,
                        "is_sensitive": item.is_sensitive,
                        "is_dynamic": item.is_dynamic,
                        "description": item.description,
                        "validation_info": item.validation_info
                    })

            return {
                "query": query,
                "timestamp": snapshot.timestamp.isoformat(),
                "result_count": len(results),
                "results": results
            }

        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to search configuration: {e}")
            raise ConfigSnapshotError(f"Failed to search configuration: {e}")

    async def validate_config(self) -> Dict[str, Any]:
        """
        验证配置的完整性和一致性

        Returns:
            Dict[str, Any]: 验证结果
        """
        if not self.generator:
            raise ConfigSnapshotError("Configuration snapshot generator not initialized")

        try:
            # 生成完整快照
            snapshot = await self.generator.generate_snapshot(include_sensitive=True)

            validation_result = {
                "timestamp": snapshot.timestamp.isoformat(),
                "total_items": snapshot.total_items,
                "valid": True,
                "warnings": [],
                "errors": [],
                "statistics": {
                    "source_distribution": {
                        source.value: count for source, count in snapshot.source_summary.items()
                    },
                    "category_distribution": {},
                    "sensitive_items": 0,
                    "dynamic_items": 0
                }
            }

            # 统计各类配置
            for group_name, group in snapshot.groups.items():
                validation_result["statistics"]["category_distribution"][group_name] = group.item_count
                validation_result["statistics"]["sensitive_items"] += group.get_sensitive_count()
                validation_result["statistics"]["dynamic_items"] += group.get_dynamic_count()

            # 检查配置一致性
            for group_name, group in snapshot.groups.items():
                for item in group.items:
                    # 检查无效的来源
                    if item.source.value not in ["default", "toml", "kv", "env"]:
                        validation_result["warnings"].append(
                            f"Configuration item {item.key} has unknown source: {item.source.value}"
                        )

                    # 检查空值
                    if item.value is None or item.value == "":
                        validation_result["warnings"].append(
                            f"Configuration item {item.key} has empty value"
                        )

            # 如果有错误，标记为无效
            if validation_result["errors"]:
                validation_result["valid"] = False

            return validation_result

        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to validate configuration: {e}")
            raise ConfigSnapshotError(f"Failed to validate configuration: {e}")

    async def export_diff(self,
                         baseline_file: Union[str, Path],
                         format: str = "table",
                         output_file: Optional[Union[str, Path]] = None) -> str:
        """
        导出当前配置与基线的差异

        Args:
            baseline_file: 基线配置文件路径
            format: 输出格式 ("json", "yaml", "table")
            output_file: 输出文件路径

        Returns:
            str: 差异报告内容
        """
        try:
            # 读取基线配置
            baseline_path = Path(baseline_file)
            if not baseline_path.exists():
                raise ConfigSnapshotError(f"Baseline file not found: {baseline_file}")

            import json
            with open(baseline_path, 'r', encoding='utf-8') as f:
                if baseline_path.suffix.lower() == '.json':
                    baseline_data = json.load(f)
                else:
                    # 简单解析，假设是键值对格式
                    baseline_data = {}
                    for line in f:
                        if '=' in line and not line.strip().startswith('#'):
                            key, value = line.split('=', 1)
                            baseline_data[key.strip()] = value.strip()

            # 生成当前配置快照
            snapshot = await self.generator.generate_snapshot(include_sensitive=True)
            current_config = {item.key: item.value for group in snapshot.groups.values() for item in group.items}

            # 计算差异
            diff = {
                "timestamp": snapshot.timestamp.isoformat(),
                "baseline_file": str(baseline_path),
                "added": {},
                "removed": {},
                "modified": {},
                "unchanged": {}
            }

            baseline_keys = set(baseline_data.keys())
            current_keys = set(current_config.keys())

            # 新增的配置
            for key in current_keys - baseline_keys:
                diff["added"][key] = current_config[key]

            # 删除的配置
            for key in baseline_keys - current_keys:
                diff["removed"][key] = baseline_data[key]

            # 修改的配置
            for key in baseline_keys & current_keys:
                if baseline_data[key] != current_config[key]:
                    diff["modified"][key] = {
                        "old": baseline_data[key],
                        "new": current_config[key]
                    }
                else:
                    diff["unchanged"][key] = current_config[key]

            # 格式化输出
            if format == "json":
                content = json.dumps(diff, indent=2, ensure_ascii=False)
            elif format == "yaml":
                try:
                    import yaml
                    content = yaml.dump(diff, default_flow_style=False, allow_unicode=True)
                except ImportError:
                    content = "# PyYAML not installed, falling back to JSON\n" + \
                             json.dumps(diff, indent=2, ensure_ascii=False)
            else:  # table
                content = self._format_diff_table(diff)

            # 输出到文件或返回字符串
            if output_file:
                await self._write_to_file(content, output_file)
                return f"Configuration diff exported to: {output_file}"
            else:
                return content

        except Exception as e:
            logger.error(f"[CONFIG_EXPORT] [ERROR] Failed to export configuration diff: {e}")
            raise ConfigSnapshotError(f"Failed to export configuration diff: {e}")

    def _format_diff_table(self, diff: Dict[str, Any]) -> str:
        """格式化差异为表格"""
        lines = []
        lines.append("=" * 100)
        lines.append(f"配置差异报告 - {diff['timestamp']}")
        lines.append(f"基线文件: {diff['baseline_file']}")
        lines.append("=" * 100)

        # 新增配置
        if diff["added"]:
            lines.append(f"\n[ADDED] New configuration ({len(diff['added'])} items):")
            lines.append("-" * 100)
            for key, value in diff["added"].items():
                lines.append(f"  {key:<50} = {value}")

        # 删除配置
        if diff["removed"]:
            lines.append(f"\nRemoved config ({len(diff['removed'])} items):")
            lines.append("-" * 100)
            for key, value in diff["removed"].items():
                lines.append(f"  {key:<50} = {value}")

        # 修改配置
        if diff["modified"]:
            lines.append(f"\nModified config ({len(diff['modified'])} items):")
            lines.append("-" * 100)
            for key, change in diff["modified"].items():
                lines.append(f"  {key:<50}")
                lines.append(f"    Old value: {change['old']}")
                lines.append(f"    New value: {change['new']}")

        # 未变更配置
        if diff["unchanged"]:
            lines.append(f"\n[UNCHANGED] Unchanged configuration ({len(diff['unchanged'])} items):")
            lines.append("-" * 100)
            for key, value in list(diff["unchanged"].items())[:10]:  # 只显示前10项
                lines.append(f"  {key:<50} = {value}")
            if len(diff["unchanged"]) > 10:
                lines.append(f"  ... {len(diff['unchanged']) - 10} more unchanged configuration items")

        return "\n".join(lines)


# 全局配置导出服务实例
_export_service: Optional[ConfigExportService] = None


def get_config_export_service() -> ConfigExportService:
    """获取全局配置导出服务实例"""
    global _export_service
    if _export_service is None:
        _export_service = ConfigExportService()
    return _export_service


# 便捷函数
async def export_config_snapshot(**kwargs) -> str:
    """便捷函数：导出配置快照"""
    service = get_config_export_service()
    return await service.export_config(**kwargs)


async def get_config_summary() -> Dict[str, Any]:
    """便捷函数：获取配置摘要"""
    service = get_config_export_service()
    return await service.get_config_summary()


async def search_config_items(query: str, **kwargs) -> Dict[str, Any]:
    """便捷函数：搜索配置项"""
    service = get_config_export_service()
    return await service.search_config(query, **kwargs)


async def validate_current_config() -> Dict[str, Any]:
    """便捷函数：验证当前配置"""
    service = get_config_export_service()
    return await service.validate_config()
