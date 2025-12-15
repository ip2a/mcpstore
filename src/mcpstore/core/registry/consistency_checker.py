"""
数据一致性检查器

用于验证内存缓存与 pykv 真相数据源之间的一致性
"""

import logging
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class ConsistencyIssue:
    """数据一致性问题"""
    issue_type: str  # 问题类型
    agent_id: str   # Agent ID
    service_name: Optional[str] = None  # 服务名称
    tool_name: Optional[str] = None  # 工具名称
    expected_value: Any = None  # 期望值
    actual_value: Any = None  # 实际值
    description: str = ""  # 问题描述


@dataclass
class ConsistencyReport:
    """一致性检查报告"""
    agent_id: str
    total_memory_services: int
    total_memory_tools: int
    total_pykv_services: int
    total_pykv_tools: int
    issues: List[ConsistencyIssue]
    is_consistent: bool
    timestamp: float

    @property
    def issue_count(self) -> int:
        return len(self.issues)

    @property
    def critical_issues(self) -> List[ConsistencyIssue]:
        """返回严重问题"""
        return [issue for issue in self.issues if issue.issue_type.startswith("CRITICAL")]


class ConsistencyChecker:
    """数据一致性检查器"""

    def __init__(self, registry: 'CoreRegistry'):
        """
        初始化一致性检查器

        Args:
            registry: CoreRegistry 实例
        """
        self.registry = registry
        logger.debug("ConsistencyChecker initialized")

    def check_agent_consistency(self, agent_id: str) -> ConsistencyReport:
        """
        检查指定 agent 的数据一致性

        Args:
            agent_id: Agent ID

        Returns:
            ConsistencyReport: 一致性检查报告
        """
        import time
        start_time = time.time()

        issues = []

        # Phase 1: 检查服务一致性
        service_issues = self._check_service_consistency(agent_id)
        issues.extend(service_issues)

        # Phase 2: 检查工具一致性
        tool_issues = self._check_tool_consistency(agent_id)
        issues.extend(tool_issues)

        # Phase 3: 统计数据
        memory_services = len(self.registry.sessions.get(agent_id, {}))
        memory_tools = len(self.registry.tool_to_session_map.get(agent_id, {}))

        pykv_services = self._count_pykv_services(agent_id)
        pykv_tools = self._count_pykv_tools(agent_id)

        # Phase 4: 生成报告
        report = ConsistencyReport(
            agent_id=agent_id,
            total_memory_services=memory_services,
            total_memory_tools=memory_tools,
            total_pykv_services=pykv_services,
            total_pykv_tools=pykv_tools,
            issues=issues,
            is_consistent=len(issues) == 0,
            timestamp=time.time() - start_time
        )

        logger.info(
            f"Consistency check completed for agent {agent_id}: "
            f"services={memory_services}/{pykv_services}, "
            f"tools={memory_tools}/{pykv_tools}, "
            f"issues={len(issues)}, "
            f"consistent={report.is_consistent}"
        )

        return report

    def _check_service_consistency(self, agent_id: str) -> List[ConsistencyIssue]:
        """检查服务一致性"""
        issues = []

        # 获取内存中的服务
        memory_services = set(self.registry.sessions.get(agent_id, {}).keys())

        # 检查内存中的服务是否在 pykv 中存在
        for service_name in memory_services:
            try:
                # 生成服务全局名称
                service_global_name = self.registry._naming.generate_service_global_name(service_name, agent_id)

                # 从 pykv 检查
                pykv_service = self.registry._sync_to_kv(
                    self.registry._service_manager.get_service(service_global_name),
                    f"consistency_check_service:{service_global_name}"
                )

                if pykv_service is None:
                    issues.append(ConsistencyIssue(
                        issue_type="CRITICAL_SERVICE_MISSING_IN_PYKV",
                        agent_id=agent_id,
                        service_name=service_name,
                        description=f"Service {service_name} exists in memory but not in pykv (truth source)",
                        actual_value="memory_only",
                        expected_value="pykv_and_memory"
                    ))
                    logger.error(f"Service consistency issue: {service_name} exists in memory but not in pykv")

            except Exception as e:
                issues.append(ConsistencyIssue(
                    issue_type="ERROR_SERVICE_CHECK_FAILED",
                    agent_id=agent_id,
                    service_name=service_name,
                    description=f"Failed to check service {service_name} in pykv: {e}",
                    actual_value="check_failed",
                    expected_value="check_successful"
                ))
                logger.error(f"Service check failed: {service_name}, error={e}")

        return issues

    def _check_tool_consistency(self, agent_id: str) -> List[ConsistencyIssue]:
        """检查工具一致性"""
        issues = []

        # 获取内存中的工具映射
        memory_tools = set(self.registry.tool_to_session_map.get(agent_id, {}).keys())

        # 检查每个工具
        for tool_name in memory_tools:
            try:
                # 从 pykv 检查工具是否存在
                pykv_tools = self.registry._sync_to_kv(
                    self.registry._state_backend.list_tools(agent_id),
                    f"consistency_check_tools:{agent_id}"
                )

                if pykv_tools and tool_name not in pykv_tools:
                    issues.append(ConsistencyIssue(
                        issue_type="CRITICAL_TOOL_MISSING_IN_PYKV",
                        agent_id=agent_id,
                        tool_name=tool_name,
                        description=f"Tool {tool_name} exists in memory but not in pykv (truth source)",
                        actual_value="memory_only",
                        expected_value="pykv_and_memory"
                    ))
                    logger.error(f"Tool consistency issue: {tool_name} exists in memory but not in pykv")

            except Exception as e:
                issues.append(ConsistencyIssue(
                    issue_type="ERROR_TOOL_CHECK_FAILED",
                    agent_id=agent_id,
                    tool_name=tool_name,
                    description=f"Failed to check tool {tool_name} in pykv: {e}",
                    actual_value="check_failed",
                    expected_value="check_successful"
                ))
                logger.error(f"Tool check failed: {tool_name}, error={e}")

        return issues

    def _count_pykv_services(self, agent_id: str) -> int:
        """统计 pykv 中的服务数量"""
        try:
            # 从实体层获取所有服务实体
            services = self.registry._sync_to_kv(
                self.registry._service_manager.list_services_by_agent(agent_id),
                f"count_pykv_services:{agent_id}"
            )
            return len(services) if services else 0
        except Exception as e:
            logger.error(f"Failed to count pykv services for agent {agent_id}: {e}")
            return 0

    def _count_pykv_tools(self, agent_id: str) -> int:
        """统计 pykv 中的工具数量"""
        try:
            # 从状态层获取所有工具
            tools = self.registry._sync_to_kv(
                self.registry._state_backend.list_tools(agent_id),
                f"count_pykv_tools:{agent_id}"
            )
            return len(tools) if tools else 0
        except Exception as e:
            logger.error(f"Failed to count pykv tools for agent {agent_id}: {e}")
            return 0

    def check_all_agents(self) -> Dict[str, ConsistencyReport]:
        """
        检查所有 agent 的一致性

        Returns:
            Dict[str, ConsistencyReport]: 所有 agent 的一致性报告
        """
        reports = {}

        # 获取所有 agent ID
        agent_ids = set()
        agent_ids.update(self.registry.sessions.keys())
        agent_ids.update(self.registry.tool_to_session_map.keys())

        for agent_id in agent_ids:
            try:
                reports[agent_id] = self.check_agent_consistency(agent_id)
            except Exception as e:
                logger.error(f"Failed to check consistency for agent {agent_id}: {e}")
                reports[agent_id] = ConsistencyReport(
                    agent_id=agent_id,
                    total_memory_services=0,
                    total_memory_tools=0,
                    total_pykv_services=0,
                    total_pykv_tools=0,
                    issues=[ConsistencyIssue(
                        issue_type="ERROR_CONSISTENCY_CHECK_FAILED",
                        agent_id=agent_id,
                        description=f"Consistency check failed: {e}"
                    )],
                    is_consistent=False,
                    timestamp=0
                )

        return reports

    def get_consistency_summary(self, reports: Dict[str, ConsistencyReport]) -> Dict[str, Any]:
        """
        生成一致性检查摘要

        Args:
            reports: 一致性检查报告字典

        Returns:
            Dict[str, Any]: 摘要信息
        """
        total_agents = len(reports)
        consistent_agents = sum(1 for report in reports.values() if report.is_consistent)
        total_issues = sum(report.issue_count for report in reports.values())
        critical_issues = sum(len(report.critical_issues) for report in reports.values())

        summary = {
            "total_agents": total_agents,
            "consistent_agents": consistent_agents,
            "inconsistent_agents": total_agents - consistent_agents,
            "consistency_rate": (consistent_agents / total_agents) if total_agents > 0 else 0,
            "total_issues": total_issues,
            "critical_issues": critical_issues,
            "timestamp": max(report.timestamp for report in reports.values()) if reports else 0
        }

        logger.info(
            f"Consistency summary: {consistent_agents}/{total_agents} agents consistent "
            f"({summary['consistency_rate']:.1%}), {total_issues} total issues"
        )

        return summary

    def fix_critical_issues(self, report: ConsistencyReport) -> Dict[str, bool]:
        """
        尝试修复关键的一致性问题

        Args:
            report: 一致性检查报告

        Returns:
            Dict[str, bool]: 修复结果，键为问题标识，值为是否修复成功
        """
        fixes = {}

        for issue in report.critical_issues:
            try:
                if issue.issue_type == "CRITICAL_SERVICE_MISSING_IN_PYKV":
                    # 尝试从内存恢复服务到 pykv
                    success = self._fix_missing_service(issue.agent_id, issue.service_name)
                    fixes[f"service_{issue.service_name}"] = success

                elif issue.issue_type == "CRITICAL_TOOL_MISSING_IN_PYKV":
                    # 尝试从内存恢复工具到 pykv
                    success = self._fix_missing_tool(issue.agent_id, issue.tool_name)
                    fixes[f"tool_{issue.tool_name}"] = success

                else:
                    fixes[issue.issue_type] = False

            except Exception as e:
                logger.error(f"Failed to fix issue {issue.issue_type}: {e}")
                fixes[issue.issue_type] = False

        return fixes

    def _fix_missing_service(self, agent_id: str, service_name: str) -> bool:
        """修复缺失的服务"""
        # 这里可以实现从内存恢复服务到 pykv 的逻辑
        # 由于这涉及复杂的重建逻辑，暂时返回 False
        logger.warning(f"Auto-fix for missing service {service_name} not implemented")
        return False

    def _fix_missing_tool(self, agent_id: str, tool_name: str) -> bool:
        """修复缺失的工具"""
        # 这里可以实现从内存恢复工具到 pykv 的逻辑
        # 由于这涉及复杂的重建逻辑，暂时返回 False
        logger.warning(f"Auto-fix for missing tool {tool_name} not implemented")
        return False


# 便捷函数
def check_registry_consistency(registry: 'CoreRegistry') -> Dict[str, Any]:
    """
    检查整个注册表的一致性

    Args:
        registry: CoreRegistry 实例

    Returns:
        Dict[str, Any]: 检查结果
    """
    checker = ConsistencyChecker(registry)
    reports = checker.check_all_agents()
    summary = checker.get_consistency_summary(reports)

    return {
        "summary": summary,
        "reports": {agent_id: {
            "is_consistent": report.is_consistent,
            "issues": report.issue_count,
            "critical_issues": len(report.critical_issues),
            "details": [issue.description for issue in report.issues]
        } for agent_id, report in reports.items()}
    }


def fix_registry_consistency(registry: 'CoreRegistry') -> Dict[str, Any]:
    """
    修复注册表的一致性问题

    Args:
        registry: CoreRegistry 实例

    Returns:
        Dict[str, Any]: 修复结果
    """
    checker = ConsistencyChecker(registry)
    reports = checker.check_all_agents()

    fixes_applied = {}
    total_issues = 0
    fixed_issues = 0

    for agent_id, report in reports.items():
        if not report.is_consistent:
            agent_fixes = checker.fix_critical_issues(report)
            fixes_applied[agent_id] = agent_fixes
            total_issues += len(report.critical_issues)
            fixed_issues += sum(1 for success in agent_fixes.values() if success)

    summary = {
        "total_critical_issues": total_issues,
        "fixed_issues": fixed_issues,
        "fix_success_rate": (fixed_issues / total_issues) if total_issues > 0 else 0,
        "fixes_applied": fixes_applied
    }

    logger.info(f"Consistency fixes applied: {fixed_issues}/{total_issues} issues fixed")

    return {
        "summary": summary,
        "fixes": fixes_applied
    }