"""
MCPStore API - Monitoring-related routes
Contains all monitoring, statistics, health check and other related API endpoints
"""

from fastapi import APIRouter
from mcpstore.core.models.common import APIResponse

from .api_decorators import handle_exceptions, get_store
from .api_models import (
    AgentsSummaryResponse, AgentStatisticsResponse, AgentServiceSummaryResponse,
    ServiceLifecycleConfig, ContentUpdateConfig, AddAlertRequest, ServiceHealthResponse, HealthSummaryResponse
)

# Create monitoring-related router
monitoring_router = APIRouter()

# === Agent statistics functionality ===
@monitoring_router.get("/agents_summary", response_model=APIResponse)
@handle_exceptions
async def get_agents_summary():
    """
    Get statistical summary information for all Agents
    
    Returns:
        APIResponse: Response containing all Agent statistical information
        
    Response Data Structure:
        {
            "total_agents": int,           # 总Agent数量
            "active_agents": int,          # 活跃Agent数量（有服务的Agent）
            "total_services": int,         # 总服务数量（包括Store和所有Agent）
            "total_tools": int,            # 总工具数量（包括Store和所有Agent）
            "store_services": int,         # Store级别服务数量
            "store_tools": int,            # Store级别工具数量
            "agents": [                    # Agent详细列表
                {
                    "agent_id": str,
                    "service_count": int,
                    "tool_count": int,
                    "healthy_services": int,
                    "unhealthy_services": int,
                    "total_tool_executions": int,
                    "last_activity": str,
                    "services": [
                        {
                            "service_name": str,
                            "service_type": str,
                            "status": str,
                            "tool_count": int,
                            "last_used": str,
                            "client_id": str
                        }
                    ]
                }
            ]
        }
    """
    try:
        store = get_store()
        
        # 调用SDK的Agent统计功能
        summary = await store.for_store().get_agents_summary_async()
        
        # 转换为API响应格式
        agents_data = []
        for agent_stats in summary.agents:
            services_data = []
            for service in agent_stats.services:
                services_data.append(AgentServiceSummaryResponse(
                    service_name=service.service_name,
                    service_type=service.service_type,
                    status=service.status.value,  # 转换枚举为字符串
                    tool_count=service.tool_count,
                    last_used=service.last_used.isoformat() if service.last_used else None,
                    client_id=service.client_id,
                    response_time=service.response_time,
                    health_details=service.health_details.dict() if service.health_details else None
                ).dict())
            
            agents_data.append(AgentStatisticsResponse(
                agent_id=agent_stats.agent_id,
                service_count=agent_stats.service_count,
                tool_count=agent_stats.tool_count,
                healthy_services=agent_stats.healthy_services,
                unhealthy_services=agent_stats.unhealthy_services,
                total_tool_executions=agent_stats.total_tool_executions,
                last_activity=agent_stats.last_activity.isoformat() if agent_stats.last_activity else None,
                services=services_data
            ).dict())
        
        response_data = AgentsSummaryResponse(
            total_agents=summary.total_agents,
            active_agents=summary.active_agents,
            total_services=summary.total_services,
            total_tools=summary.total_tools,
            store_services=summary.store_services,
            store_tools=summary.store_tools,
            agents=agents_data
        ).dict()
        
        return APIResponse(
            success=True,
            data=response_data,
            message=f"Agents summary retrieved successfully. Found {summary.total_agents} agents, {summary.active_agents} active."
        )
        
    except Exception as e:
        return APIResponse(
            success=False,
            data={
                "total_agents": 0,
                "active_agents": 0,
                "total_services": 0,
                "total_tools": 0,
                "store_services": 0,
                "store_tools": 0,
                "agents": []
            },
            message=f"Failed to get agents summary: {str(e)}"
        )

# === 监控配置管理 ===
@monitoring_router.get("/monitoring/config", response_model=APIResponse)
@handle_exceptions
async def get_monitoring_config():
    """获取监控配置（兼容旧接口）"""
    try:
        store = get_store()

        # 返回一个基本的监控配置信息
        # 注意：这是为了兼容性，实际配置现在由生命周期管理器管理
        config = {
            "status": "deprecated",
            "message": "Monitoring configuration has been replaced by lifecycle management",
            "redirect_to": "/lifecycle/config"
        }

        return APIResponse(
            success=True,
            data=config,
            message="Legacy monitoring configuration (deprecated, use /lifecycle/config instead)"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get monitoring configuration: {str(e)}"
        )

@monitoring_router.post("/lifecycle/config", response_model=APIResponse)
@handle_exceptions
async def update_lifecycle_config(config: ServiceLifecycleConfig):
    """更新生命周期配置"""
    try:
        store = get_store()

        # 转换为字典格式，过滤None值
        config_dict = {k: v for k, v in config.dict().items() if v is not None}

        # 注意：这里需要实现新的配置更新方法
        # result = await store.for_store().update_lifecycle_config_async(config_dict)

        # 临时返回成功，实际配置更新功能需要后续实现
        return APIResponse(
            success=True,
            data=config_dict,
            message="Lifecycle configuration update received (implementation pending)"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to update monitoring configuration: {str(e)}"
        )

# === 告警管理 ===
@monitoring_router.post("/monitoring/alerts", response_model=APIResponse)
@handle_exceptions
async def add_alert(alert: AddAlertRequest):
    """添加告警"""
    try:
        store = get_store()
        
        alert_data = {
            "type": alert.type,
            "title": alert.title,
            "message": alert.message,
            "service_name": alert.service_name
        }
        
        result = await store.for_store().add_alert_async(alert_data)
        
        return APIResponse(
            success=bool(result),
            data=result,
            message="Alert added successfully" if result else "Failed to add alert"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to add alert: {str(e)}"
        )

@monitoring_router.get("/monitoring/alerts", response_model=APIResponse)
@handle_exceptions
async def get_alerts(limit: int = 50):
    """获取告警列表"""
    try:
        store = get_store()
        alerts = await store.for_store().get_alerts_async(limit)
        
        return APIResponse(
            success=True,
            data=alerts,
            message=f"Retrieved {len(alerts) if isinstance(alerts, list) else 0} alerts"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to get alerts: {str(e)}"
        )

@monitoring_router.delete("/monitoring/alerts", response_model=APIResponse)
@handle_exceptions
async def clear_alerts():
    """清除所有告警"""
    try:
        store = get_store()
        result = await store.for_store().clear_alerts_async()
        
        return APIResponse(
            success=bool(result),
            data=result,
            message="All alerts cleared successfully" if result else "Failed to clear alerts"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to clear alerts: {str(e)}"
        )

# === 性能监控 ===
@monitoring_router.get("/monitoring/performance", response_model=APIResponse)
@handle_exceptions
async def get_performance_metrics():
    """获取性能指标"""
    try:
        store = get_store()
        metrics = await store.for_store().get_performance_metrics_async()
        
        return APIResponse(
            success=True,
            data=metrics,
            message="Performance metrics retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get performance metrics: {str(e)}"
        )

@monitoring_router.get("/monitoring/usage_stats", response_model=APIResponse)
@handle_exceptions
async def get_usage_statistics():
    """获取使用统计"""
    try:
        store = get_store()
        stats = await store.for_store().get_usage_stats_async()
        
        return APIResponse(
            success=True,
            data=stats,
            message="Usage statistics retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get usage statistics: {str(e)}"
        )

# === 健康状态管理 ===
@monitoring_router.get("/health/summary", response_model=APIResponse)
@handle_exceptions
async def get_health_summary():
    """获取所有服务的生命周期状态汇总"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        lifecycle_manager = orchestrator.lifecycle_manager

        # 统计各状态的服务数量
        state_counts = {
            "initializing": 0,
            "healthy": 0,
            "warning": 0,
            "reconnecting": 0,
            "unreachable": 0,
            "disconnecting": 0,
            "disconnected": 0
        }

        services_health = {}
        total_services = 0

        # 🔧 修复：使用lifecycle_manager的service_states而不是registry的废弃字段
        for agent_id, services in lifecycle_manager.service_states.items():
            for service_name, state in services.items():
                total_services += 1
                state_str = state.value
                state_counts[state_str] += 1

                # 获取状态元数据
                metadata = lifecycle_manager.get_service_metadata(agent_id, service_name)

                # 🔧 改进：添加元数据存在性检查
                if metadata:
                    services_health[f"{agent_id}:{service_name}"] = ServiceHealthResponse(
                        service_name=service_name,
                        status=state_str,
                        response_time=metadata.response_time or 0.0,
                        last_check_time=metadata.last_success_time.timestamp() if metadata.last_success_time else 0.0,
                        consecutive_failures=metadata.consecutive_failures,
                        consecutive_successes=metadata.consecutive_successes,
                        reconnect_attempts=metadata.reconnect_attempts,
                        state_entered_time=metadata.state_entered_time.isoformat() if metadata.state_entered_time else None,
                        next_retry_time=metadata.next_retry_time.isoformat() if metadata.next_retry_time else None,
                        error_message=metadata.error_message,
                        details={
                            "agent_id": agent_id,
                            "disconnect_reason": metadata.disconnect_reason,
                            "has_metadata": True
                        }
                    ).dict()
                else:
                    # 没有元数据的服务（仅配置服务）
                    services_health[f"{agent_id}:{service_name}"] = {
                        "service_name": service_name,
                        "status": state_str,
                        "response_time": 0.0,
                        "last_check_time": 0.0,
                        "consecutive_failures": 0,
                        "consecutive_successes": 0,
                        "reconnect_attempts": 0,
                        "state_entered_time": None,
                        "next_retry_time": None,
                        "error_message": None,
                        "details": {
                            "agent_id": agent_id,
                            "has_metadata": False,
                            "note": "Service exists in configuration but is not activated"
                        }
                    }

        response_data = HealthSummaryResponse(
            total_services=total_services,
            initializing_count=state_counts["initializing"],
            healthy_count=state_counts["healthy"],
            warning_count=state_counts["warning"],
            reconnecting_count=state_counts["reconnecting"],
            unreachable_count=state_counts["unreachable"],
            disconnecting_count=state_counts["disconnecting"],
            disconnected_count=state_counts["disconnected"],
            services=services_health
        ).dict()

        return APIResponse(
            success=True,
            data=response_data,
            message=f"Lifecycle status summary retrieved successfully. {total_services} services tracked."
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={
                "total_services": 0,
                "initializing_count": 0,
                "healthy_count": 0,
                "warning_count": 0,
                "reconnecting_count": 0,
                "unreachable_count": 0,
                "disconnecting_count": 0,
                "disconnected_count": 0,
                "services": {}
            },
            message=f"Failed to get lifecycle status summary: {str(e)}"
        )

@monitoring_router.get("/health/service/{service_name}", response_model=APIResponse)
@handle_exceptions
async def get_service_health(service_name: str, agent_id: str = None):
    """获取特定服务的详细生命周期状态"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        lifecycle_manager = orchestrator.lifecycle_manager

        # 确定agent_id
        target_agent_id = agent_id or orchestrator.client_manager.global_agent_store_id

        # 🔧 改进：检查服务是否存在，支持跨agent查找
        state = lifecycle_manager.get_service_state(target_agent_id, service_name)
        metadata = lifecycle_manager.get_service_metadata(target_agent_id, service_name)

        # 如果在指定agent中没有找到，尝试在所有agent中查找
        if state is None:
            for agent_id in lifecycle_manager.service_states:
                if service_name in lifecycle_manager.service_states[agent_id]:
                    target_agent_id = agent_id
                    state = lifecycle_manager.get_service_state(agent_id, service_name)
                    metadata = lifecycle_manager.get_service_metadata(agent_id, service_name)
                    break

        if state is None:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found in any agent"
            )

        response_data = ServiceHealthResponse(
            service_name=service_name,
            status=state.value,
            response_time=metadata.response_time or 0.0,
            last_check_time=metadata.last_success_time.timestamp() if metadata.last_success_time else 0.0,
            consecutive_failures=metadata.consecutive_failures,
            consecutive_successes=metadata.consecutive_successes,
            reconnect_attempts=metadata.reconnect_attempts,
            state_entered_time=metadata.state_entered_time.isoformat() if metadata.state_entered_time else None,
            next_retry_time=metadata.next_retry_time.isoformat() if metadata.next_retry_time else None,
            error_message=metadata.error_message,
            details={
                "agent_id": target_agent_id,
                "disconnect_reason": metadata.disconnect_reason,
                "last_failure_time": metadata.last_failure_time.isoformat() if metadata.last_failure_time else None
            }
        ).dict()

        return APIResponse(
            success=True,
            data=response_data,
            message=f"Lifecycle status retrieved for service '{service_name}' (agent: {target_agent_id})"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get lifecycle status for service '{service_name}': {str(e)}"
        )

@monitoring_router.post("/health/check/{service_name}", response_model=APIResponse)
@handle_exceptions
async def trigger_health_check(service_name: str):
    """手动触发特定服务的健康检查"""
    try:
        store = get_store()

        # 从Orchestrator触发健康检查
        orchestrator = store.orchestrator
        health_result = await orchestrator.check_service_health_detailed(service_name)

        response_data = ServiceHealthResponse(
            service_name=service_name,
            status=health_result.status.value,
            response_time=health_result.response_time,
            last_check_time=health_result.timestamp,
            consecutive_failures=health_result.details.get("consecutive_failures", 0),
            average_response_time=health_result.details.get("avg_response_time", 0.0),
            adaptive_timeout=0.0,  # 会在下次获取时更新
            error_message=health_result.error_message,
            details=health_result.details
        ).dict()

        return APIResponse(
            success=True,
            data=response_data,
            message=f"Health check completed for service '{service_name}'. Status: {health_result.status.value}"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to check health for service '{service_name}': {str(e)}"
        )

@monitoring_router.post("/tools/refresh", response_model=APIResponse)
@handle_exceptions
async def refresh_all_tools():
    """手动刷新所有服务的内容（工具、资源、提示词）"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        content_manager = orchestrator.content_manager

        if not content_manager.is_running:
            return APIResponse(
                success=False,
                data={},
                message="Content manager is not running"
            )

        # 获取所有需要更新的服务
        services_to_update = []
        for agent_id, services in content_manager.content_snapshots.items():
            for service_name in services.keys():
                services_to_update.append((agent_id, service_name))

        if not services_to_update:
            return APIResponse(
                success=True,
                data={
                    "updated_services": 0,
                    "total_services": 0,
                    "results": {}
                },
                message="No services found for content refresh"
            )

        # 并发更新所有服务内容
        results = {}
        for agent_id, service_name in services_to_update:
            success = await content_manager.force_update_service_content(agent_id, service_name)
            results[f"{agent_id}:{service_name}"] = success

        success_count = sum(1 for success in results.values() if success)
        total_count = len(results)

        return APIResponse(
            success=True,
            data={
                "updated_services": success_count,
                "total_services": total_count,
                "results": results
            },
            message=f"Content refresh completed: {success_count}/{total_count} services updated successfully"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to refresh content: {str(e)}"
        )

@monitoring_router.post("/tools/refresh/{service_name}", response_model=APIResponse)
@handle_exceptions
async def refresh_service_tools(service_name: str, agent_id: str = None):
    """手动刷新特定服务的内容（工具、资源、提示词）"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        content_manager = orchestrator.content_manager

        if not content_manager.is_running:
            return APIResponse(
                success=False,
                data={},
                message="Content manager is not running"
            )

        # 确定agent_id
        target_agent_id = agent_id or orchestrator.client_manager.global_agent_store_id

        # 检查服务是否在监控中
        snapshot = content_manager.get_service_snapshot(target_agent_id, service_name)
        if not snapshot:
            return APIResponse(
                success=False,
                data={"service_name": service_name, "agent_id": target_agent_id},
                message=f"Service '{service_name}' not found in content monitoring for agent '{target_agent_id}'"
            )

        # 手动更新特定服务的内容
        success = await content_manager.force_update_service_content(target_agent_id, service_name)

        if success:
            # 获取更新后的快照
            updated_snapshot = content_manager.get_service_snapshot(target_agent_id, service_name)
            return APIResponse(
                success=True,
                data={
                    "service_name": service_name,
                    "agent_id": target_agent_id,
                    "tools_count": updated_snapshot.tools_count if updated_snapshot else 0,
                    "last_updated": updated_snapshot.last_updated.isoformat() if updated_snapshot else None
                },
                message=f"Content refreshed successfully for service '{service_name}' (agent: {target_agent_id})"
            )
        else:
            return APIResponse(
                success=False,
                data={"service_name": service_name, "agent_id": target_agent_id},
                message=f"Failed to refresh content for service '{service_name}' (agent: {target_agent_id})"
            )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to refresh content for service '{service_name}': {str(e)}"
        )

# === 生命周期管理API ===
@monitoring_router.post("/lifecycle/disconnect/{service_name}", response_model=APIResponse)
@handle_exceptions
async def graceful_disconnect_service(service_name: str, agent_id: str = None, reason: str = "user_requested"):
    """优雅断连指定服务"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        lifecycle_manager = orchestrator.lifecycle_manager

        # 确定agent_id
        target_agent_id = agent_id or orchestrator.client_manager.global_agent_store_id

        # 检查服务是否存在
        state = lifecycle_manager.get_service_state(target_agent_id, service_name)
        if state is None:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found for agent '{target_agent_id}'"
            )

        # 执行优雅断连
        await lifecycle_manager.graceful_disconnect(target_agent_id, service_name, reason)

        return APIResponse(
            success=True,
            data={
                "service_name": service_name,
                "agent_id": target_agent_id,
                "reason": reason,
                "previous_state": state.value
            },
            message=f"Graceful disconnect initiated for service '{service_name}' (agent: {target_agent_id})"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to disconnect service '{service_name}': {str(e)}"
        )

@monitoring_router.get("/lifecycle/config", response_model=APIResponse)
@handle_exceptions
async def get_lifecycle_config():
    """获取当前生命周期配置"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        lifecycle_manager = orchestrator.lifecycle_manager
        content_manager = orchestrator.content_manager

        lifecycle_config = {
            "warning_failure_threshold": lifecycle_manager.config.warning_failure_threshold,
            "reconnecting_failure_threshold": lifecycle_manager.config.reconnecting_failure_threshold,
            "max_reconnect_attempts": lifecycle_manager.config.max_reconnect_attempts,
            "base_reconnect_delay": lifecycle_manager.config.base_reconnect_delay,
            "max_reconnect_delay": lifecycle_manager.config.max_reconnect_delay,
            "long_retry_interval": lifecycle_manager.config.long_retry_interval,
            "normal_heartbeat_interval": lifecycle_manager.config.normal_heartbeat_interval,
            "warning_heartbeat_interval": lifecycle_manager.config.warning_heartbeat_interval,
            "initialization_timeout": lifecycle_manager.config.initialization_timeout,
            "disconnection_timeout": lifecycle_manager.config.disconnection_timeout
        }

        content_config = {
            "tools_update_interval": content_manager.config.tools_update_interval,
            "resources_update_interval": content_manager.config.resources_update_interval,
            "prompts_update_interval": content_manager.config.prompts_update_interval,
            "max_concurrent_updates": content_manager.config.max_concurrent_updates,
            "update_timeout": content_manager.config.update_timeout,
            "max_consecutive_failures": content_manager.config.max_consecutive_failures,
            "failure_backoff_multiplier": content_manager.config.failure_backoff_multiplier
        }

        return APIResponse(
            success=True,
            data={
                "lifecycle_config": lifecycle_config,
                "content_config": content_config
            },
            message="Lifecycle configuration retrieved successfully"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get lifecycle configuration: {str(e)}"
        )

@monitoring_router.get("/content/snapshot/{service_name}", response_model=APIResponse)
@handle_exceptions
async def get_service_content_snapshot(service_name: str, agent_id: str = None):
    """获取服务内容快照"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        content_manager = orchestrator.content_manager

        # 确定agent_id
        target_agent_id = agent_id or orchestrator.client_manager.global_agent_store_id

        # 获取内容快照
        snapshot = content_manager.get_service_snapshot(target_agent_id, service_name)
        if not snapshot:
            return APIResponse(
                success=False,
                data={},
                message=f"Content snapshot not found for service '{service_name}' (agent: {target_agent_id})"
            )

        return APIResponse(
            success=True,
            data={
                "service_name": snapshot.service_name,
                "agent_id": snapshot.agent_id,
                "tools_count": snapshot.tools_count,
                "tools_hash": snapshot.tools_hash,
                "resources_count": snapshot.resources_count,
                "resources_hash": snapshot.resources_hash,
                "prompts_count": snapshot.prompts_count,
                "prompts_hash": snapshot.prompts_hash,
                "last_updated": snapshot.last_updated.isoformat()
            },
            message=f"Content snapshot retrieved for service '{service_name}' (agent: {target_agent_id})"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get content snapshot for service '{service_name}': {str(e)}"
        )

@monitoring_router.get("/content/snapshots", response_model=APIResponse)
@handle_exceptions
async def get_all_content_snapshots():
    """获取所有服务的内容快照"""
    try:
        store = get_store()
        orchestrator = store.orchestrator
        content_manager = orchestrator.content_manager

        all_snapshots = {}
        total_services = 0

        for agent_id, services in content_manager.content_snapshots.items():
            for service_name, snapshot in services.items():
                total_services += 1
                key = f"{agent_id}:{service_name}"
                all_snapshots[key] = {
                    "service_name": snapshot.service_name,
                    "agent_id": snapshot.agent_id,
                    "tools_count": snapshot.tools_count,
                    "tools_hash": snapshot.tools_hash[:8] + "..." if snapshot.tools_hash else "",
                    "resources_count": snapshot.resources_count,
                    "prompts_count": snapshot.prompts_count,
                    "last_updated": snapshot.last_updated.isoformat()
                }

        return APIResponse(
            success=True,
            data={
                "total_services": total_services,
                "snapshots": all_snapshots
            },
            message=f"All content snapshots retrieved successfully. {total_services} services tracked."
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get content snapshots: {str(e)}"
        )

@monitoring_router.get("/tools/update_status", response_model=APIResponse)
@handle_exceptions
async def get_tools_update_status():
    """获取工具更新状态"""
    try:
        store = get_store()
        orchestrator = store.orchestrator

        if not orchestrator.tools_update_monitor:
            return APIResponse(
                success=True,
                data={
                    "enabled": False,
                    "message": "Tools update monitor is not enabled"
                },
                message="Tools update monitoring is disabled"
            )

        status = orchestrator.tools_update_monitor.get_update_status()

        return APIResponse(
            success=True,
            data=status,
            message="Tools update status retrieved successfully"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get tools update status: {str(e)}"
        )
