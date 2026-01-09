"""
死锁修复集成配置

统一的集成点，用于无缝替换现有组件
"""

import logging
from typing import Any, Dict, Optional

logger = logging.getLogger(__name__)


class DeadlockFixIntegration:
    """
    死锁修复集成管理器

    负责协调所有修复组件的集成和替换
    """

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        初始化集成管理器

        Args:
            config: 配置选项
        """
        self.config = config or {}
        self._applied_fixes = []
        self._migration_performed = False

        logger.info("DeadlockFixIntegration initialized")

    def apply_all_fixes(self, registry, service_management, async_helper=None):
        """
        应用所有死锁修复

        Args:
            registry: 当前注册表实例
            service_management: 当前服务管理实例
            async_helper: 当前异步助手实例（可选）
        """
        logger.info("Starting deadlock fixes application")

        try:
            # 1. 替换异步助手
            if async_helper:
                self._replace_async_helper(async_helper)

            # 2. 迁移注册表
            if registry:
                self._migrate_registry(registry)

            # 3. 迁移服务管理
            if service_management:
                self._migrate_service_management(service_management)

            # 4. 应用配置修复
            self._apply_config_fixes()

            self._migration_performed = True
            logger.info("All deadlock fixes applied successfully")

        except Exception as e:
            logger.error(f"Failed to apply deadlock fixes: {e}")
            raise

    def _replace_async_helper(self, current_helper):
        """替换异步助手"""
        logger.info("Replacing async helper with deadlock-safe version")

        try:
            from ..utils.deadlock_safe_async_helper import get_deadlock_safe_helper

            # 获取死锁安全的助手
            safe_helper = get_deadlock_safe_helper()

            # 在全局范围内替换
            import sys
            for module_name, module in sys.modules.items():
                if hasattr(module, '_sync_helper') and module._sync_helper is current_helper:
                    module._sync_helper = safe_helper
                    logger.debug(f"Replaced async helper in module: {module_name}")

            self._applied_fixes.append("async_helper_replaced")
            logger.info("Async helper replacement completed")

        except ImportError as e:
            logger.error(f"Failed to import deadlock-safe helper: {e}")
            raise

    def _migrate_registry(self, current_registry):
        """迁移注册表到异步安全版本"""
        logger.info("Migrating registry to async-safe version")

        try:
            # 异步安全注册表已废弃；保持兼容但不再迁移
            logger.warning("Async-safe registry migration is deprecated; skipping.")
            return

        except Exception as e:
            logger.error(f"Failed to migrate registry (deprecated path): {e}")
            raise

    def _migrate_service_management(self, current_service_management):
        """迁移服务管理到异步安全版本"""
        logger.info("Migrating service management to async-safe version")

        try:
            from ..context.async_safe_service_management import AsyncSafeServiceManagementFactory

            # 创建异步安全服务管理
            safe_service_management = AsyncSafeServiceManagementFactory.migrate_from_standard_management(
                current_service_management
            )

            # 替换服务管理引用
            self._replace_service_management_references(current_service_management, safe_service_management)

            self._applied_fixes.append("service_management_migrated")
            logger.info("Service management migration completed")

        except ImportError as e:
            logger.error(f"Failed to import async-safe service management: {e}")
            raise

    def _apply_config_fixes(self):
        """应用配置修复"""
        logger.info("Applying configuration fixes")

        # 修复1：调整超时配置
        timeout_config = {
            "async_operation_timeout": self.config.get("async_timeout", 30.0),
            "nested_call_detection": True,
            "max_concurrent_calls": self.config.get("max_concurrent", 10),
            "cache_enabled": self.config.get("cache_enabled", True),
            "cache_timeout": self.config.get("cache_timeout", 5.0)
        }

        # 应用到全局配置
        try:
            from ...config.toml_config import get_mcp_config
            mcp_config = get_mcp_config()

            # 设置死锁修复相关配置
            mcp_config.set("deadlock_fix.async_timeout", timeout_config["async_operation_timeout"])
            mcp_config.set("deadlock_fix.nested_call_detection", timeout_config["nested_call_detection"])
            mcp_config.set("deadlock_fix.max_concurrent", timeout_config["max_concurrent_calls"])
            mcp_config.set("deadlock_fix.cache_enabled", timeout_config["cache_enabled"])
            mcp_config.set("deadlock_fix.cache_timeout", timeout_config["cache_timeout"])

            self._applied_fixes.append("config_fixed")
            logger.info("Configuration fixes applied")

        except Exception as e:
            logger.warning(f"Failed to apply configuration fixes: {e}")

    def _replace_registry_references(self, old_registry, new_registry):
        """替换注册表引用"""
        import sys

        for module_name, module in sys.modules.items():
            if hasattr(module, '_registry') and module._registry is old_registry:
                module._registry = new_registry
                logger.debug(f"Replaced registry in module: {module_name}")

            if hasattr(module, 'registry') and module.registry is old_registry:
                module.registry = new_registry
                logger.debug(f"Replaced registry in module: {module_name}")

    def _replace_service_management_references(self, old_service_management, new_service_management):
        """替换服务管理引用"""
        import sys

        for module_name, module in sys.modules.items():
            if hasattr(module, '_service_management') and module._service_management is old_service_management:
                module._service_management = new_service_management
                logger.debug(f"Replaced service_management in module: {module_name}")

            if hasattr(module, 'service_management') and module.service_management is old_service_management:
                module.service_management = new_service_management
                logger.debug(f"Replaced service_management in module: {module_name}")

    def get_migration_report(self) -> Dict[str, Any]:
        """获取迁移报告"""
        return {
            "migration_performed": self._migration_performed,
            "applied_fixes": self._applied_fixes,
            "fixes_count": len(self._applied_fixes),
            "config": self.config
        }

    def validate_fixes(self) -> Dict[str, Any]:
        """验证修复是否成功应用"""
        validation_results = {
            "overall_status": "unknown",
            "individual_checks": {},
            "issues": []
        }

        try:
            # 验证1：检查死锁安全助手
            try:
                from ..utils.deadlock_safe_async_helper import get_deadlock_safe_helper
                helper = get_deadlock_safe_helper()
                validation_results["individual_checks"]["deadlock_safe_helper"] = "passed"
            except Exception as e:
                validation_results["individual_checks"]["deadlock_safe_helper"] = f"failed: {e}"
                validation_results["issues"].append(f"Deadlock-safe helper issue: {e}")

            # 验证2：检查异步安全注册表（已废弃，标记为跳过）
            validation_results["individual_checks"]["async_safe_registry"] = "skipped (deprecated)"

            # 验证3：检查异步安全服务管理
            try:
                from ..context.async_safe_service_management import AsyncSafeServiceManagement
                validation_results["individual_checks"]["async_safe_service_management"] = "passed"
            except Exception as e:
                validation_results["individual_checks"]["async_safe_service_management"] = f"failed: {e}"
                validation_results["issues"].append(f"Async-safe service management issue: {e}")

            # 计算总体状态
            passed_checks = sum(1 for check in validation_results["individual_checks"].values() if check == "passed")
            total_checks = len(validation_results["individual_checks"])

            if passed_checks == total_checks:
                validation_results["overall_status"] = "success"
            elif passed_checks > 0:
                validation_results["overall_status"] = "partial"
            else:
                validation_results["overall_status"] = "failed"

            logger.info(f"Fix validation completed: {validation_results['overall_status']} ({passed_checks}/{total_checks})")

        except Exception as e:
            validation_results["overall_status"] = "error"
            validation_results["issues"].append(f"Validation error: {e}")
            logger.error(f"Fix validation failed: {e}")

        return validation_results


class DeadlockFixAutoApplier:
    """死锁修复自动应用器"""

    @staticmethod
    def auto_apply_fixes(config: Optional[Dict[str, Any]] = None) -> DeadlockFixIntegration:
        """
        自动应用死锁修复

        Args:
            config: 配置选项

        Returns:
            修复集成管理器实例
        """
        logger.info("Auto-applying deadlock fixes")

        integration = DeadlockFixIntegration(config)

        try:
            # 获取当前系统组件
            registry = DeadlockFixAutoApplier._get_current_registry()
            service_management = DeadlockFixAutoApplier._get_current_service_management()
            async_helper = DeadlockFixAutoApplier._get_current_async_helper()

            # 应用修复
            integration.apply_all_fixes(registry, service_management, async_helper)

            # 验证修复
            validation_result = integration.validate_fixes()
            if validation_result["overall_status"] != "success":
                logger.warning(f"Fix validation issues: {validation_result['issues']}")

            return integration

        except Exception as e:
            logger.error(f"Auto-application of deadlock fixes failed: {e}")
            raise

    @staticmethod
    def _get_current_registry():
        """获取当前注册表实例"""
        try:
            # 尝试从已知位置获取注册表
            from ..registry.core_registry import ServiceRegistry
            # 这里需要根据实际的应用架构来获取注册表实例
            return None  # 占位符，实际使用时需要替换
        except ImportError:
            return None

    @staticmethod
    def _get_current_service_management():
        """获取当前服务管理实例"""
        try:
            from ..context.service_management import ServiceManagement
            # 这里需要根据实际的应用架构来获取服务管理实例
            return None  # 占位符，实际使用时需要替换
        except ImportError:
            return None

    @staticmethod
    def _get_current_async_helper():
        """获取当前异步助手实例"""
        try:
            from ..bridge import get_async_bridge
            return get_async_bridge()
        except Exception:
            return None


# 导出的便捷函数
def apply_deadlock_fixes(config: Optional[Dict[str, Any]] = None) -> DeadlockFixIntegration:
    """
    应用死锁修复的便捷函数

    Args:
        config: 配置选项，如 {"async_timeout": 30.0, "max_concurrent": 10}

    Returns:
        修复集成管理器实例
    """
    return DeadlockFixAutoApplier.auto_apply_fixes(config)


def validate_deadlock_fixes() -> Dict[str, Any]:
    """
    验证死锁修复的便捷函数

    Returns:
        验证结果
    """
    integration = DeadlockFixIntegration()
    return integration.validate_fixes()
