"""
监控面板页面
"""

import streamlit as st
from typing import Dict
import time
from datetime import datetime

from utils.helpers import (
    show_success_message, show_error_message, show_info_message,
    format_json
)

def show():
    """显示监控面板页面"""
    st.header("📊 监控面板")
    
    # 创建标签页
    tab1, tab2, tab3 = st.tabs(["📈 系统状态", "🔧 监控配置", "📋 详细统计"])
    
    with tab1:
        show_system_status()
    
    with tab2:
        show_monitoring_config()
    
    with tab3:
        show_detailed_stats()

def show_system_status():
    """显示系统状态"""
    st.subheader("📈 实时系统状态")
    
    # 自动刷新控制
    col1, col2, col3 = st.columns([1, 1, 2])
    
    with col1:
        auto_refresh = st.checkbox("自动刷新", value=False)
    
    with col2:
        if st.button("🔄 手动刷新", key="monitoring_manual_refresh"):
            st.rerun()
    
    # 自动刷新逻辑
    if auto_refresh:
        time.sleep(5)
        st.rerun()
    
    # 获取监控状态
    api_client = st.session_state.api_client
    monitoring_response = api_client.get_monitoring_status()
    
    if not monitoring_response:
        show_error_message("无法获取监控状态")
        return
    
    monitoring_data = monitoring_response.get('data', {})
    
    # 系统概览指标
    st.markdown("#### 🎯 系统概览")
    
    col1, col2, col3, col4 = st.columns(4)
    
    # 服务统计
    service_stats = monitoring_data.get('service_statistics', {})
    
    with col1:
        total_services = service_stats.get('total_services', 0)
        st.metric("总服务数", total_services)
    
    with col2:
        healthy_services = service_stats.get('healthy_services', 0)
        st.metric("健康服务", healthy_services)
    
    with col3:
        unhealthy_services = service_stats.get('unhealthy_services', 0)
        st.metric("异常服务", unhealthy_services)
    
    with col4:
        health_percentage = service_stats.get('health_percentage', 0)
        st.metric("健康率", f"{health_percentage:.1f}%")
    
    # 监控任务状态
    st.markdown("#### 🔧 监控任务状态")
    
    monitoring_tasks = monitoring_data.get('monitoring_tasks', {})
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("**任务状态**:")
        
        heartbeat_active = monitoring_tasks.get('heartbeat_active', False)
        heartbeat_icon = "🟢" if heartbeat_active else "🔴"
        st.write(f"{heartbeat_icon} 心跳检查: {'运行中' if heartbeat_active else '已停止'}")
        
        reconnection_active = monitoring_tasks.get('reconnection_active', False)
        reconnection_icon = "🟢" if reconnection_active else "🔴"
        st.write(f"{reconnection_icon} 智能重连: {'运行中' if reconnection_active else '已停止'}")
        
        cleanup_active = monitoring_tasks.get('cleanup_active', False)
        cleanup_icon = "🟢" if cleanup_active else "🔴"
        st.write(f"{cleanup_icon} 资源清理: {'运行中' if cleanup_active else '已停止'}")
    
    with col2:
        st.markdown("**任务间隔**:")
        
        heartbeat_interval = monitoring_tasks.get('heartbeat_interval_seconds', 0)
        st.write(f"⏱️ 心跳间隔: {heartbeat_interval}秒")
        
        reconnection_interval = monitoring_tasks.get('reconnection_interval_seconds', 0)
        st.write(f"🔄 重连间隔: {reconnection_interval}秒")
        
        cleanup_interval = monitoring_tasks.get('cleanup_interval_seconds', 0)
        st.write(f"🧹 清理间隔: {cleanup_interval}秒")
    
    # 重连队列状态
    st.markdown("#### 🔄 智能重连队列")
    
    reconnection_queue = monitoring_data.get('reconnection_queue', {})
    
    col1, col2, col3 = st.columns(3)
    
    with col1:
        total_entries = reconnection_queue.get('total_entries', 0)
        st.metric("队列总数", total_entries)
    
    with col2:
        ready_for_retry = reconnection_queue.get('ready_for_retry', 0)
        st.metric("待重试", ready_for_retry)
    
    with col3:
        # 按优先级显示
        by_priority = reconnection_queue.get('by_priority', {})
        high_priority = by_priority.get('HIGH', 0) + by_priority.get('CRITICAL', 0)
        st.metric("高优先级", high_priority)
    
    # 优先级分布图表
    if by_priority:
        st.markdown("**优先级分布**:")
        st.bar_chart(by_priority)
    
    # 资源限制
    st.markdown("#### 📊 资源限制")
    
    resource_limits = monitoring_data.get('resource_limits', {})
    
    col1, col2, col3 = st.columns(3)
    
    with col1:
        max_queue_size = resource_limits.get('max_reconnection_queue_size', 0)
        current_queue = reconnection_queue.get('total_entries', 0)
        queue_usage = (current_queue / max_queue_size * 100) if max_queue_size > 0 else 0
        st.metric("队列使用率", f"{queue_usage:.1f}%", f"{current_queue}/{max_queue_size}")
    
    with col2:
        max_history_hours = resource_limits.get('max_heartbeat_history_hours', 0)
        st.metric("心跳历史", f"{max_history_hours}小时")
    
    with col3:
        http_timeout = resource_limits.get('http_timeout_seconds', 0)
        st.metric("HTTP超时", f"{http_timeout}秒")

def show_monitoring_config():
    """显示监控配置"""
    st.subheader("🔧 监控配置管理")
    
    # 获取当前配置
    api_client = st.session_state.api_client
    monitoring_response = api_client.get_monitoring_status()
    
    if not monitoring_response:
        show_error_message("无法获取当前配置")
        return
    
    current_config = monitoring_response.get('data', {})
    monitoring_tasks = current_config.get('monitoring_tasks', {})
    resource_limits = current_config.get('resource_limits', {})
    
    # 配置表单
    with st.form("monitoring_config_form"):
        st.markdown("#### ⏱️ 任务间隔配置")
        
        col1, col2 = st.columns(2)
        
        with col1:
            heartbeat_interval = st.number_input(
                "心跳检查间隔 (秒)",
                min_value=10,
                max_value=300,
                value=int(monitoring_tasks.get('heartbeat_interval_seconds', 30)),
                help="心跳检查的时间间隔"
            )
            
            reconnection_interval = st.number_input(
                "重连尝试间隔 (秒)",
                min_value=10,
                max_value=600,
                value=int(monitoring_tasks.get('reconnection_interval_seconds', 45)),
                help="智能重连的时间间隔"
            )
        
        with col2:
            cleanup_interval_hours = st.number_input(
                "资源清理间隔 (小时)",
                min_value=1,
                max_value=24,
                value=int(monitoring_tasks.get('cleanup_interval_seconds', 3600) / 3600),
                help="资源清理的时间间隔"
            )
            
            http_timeout = st.number_input(
                "HTTP超时时间 (秒)",
                min_value=1,
                max_value=30,
                value=int(resource_limits.get('http_timeout_seconds', 5)),
                help="HTTP请求的超时时间"
            )
        
        st.markdown("#### 📊 资源限制配置")
        
        col1, col2 = st.columns(2)
        
        with col1:
            max_queue_size = st.number_input(
                "最大重连队列大小",
                min_value=10,
                max_value=200,
                value=int(resource_limits.get('max_reconnection_queue_size', 30)),
                help="智能重连队列的最大大小"
            )
        
        with col2:
            max_history_hours = st.number_input(
                "心跳历史保留时间 (小时)",
                min_value=1,
                max_value=168,
                value=int(resource_limits.get('max_heartbeat_history_hours', 24)),
                help="心跳历史数据的保留时间"
            )
        
        # 提交按钮
        col1, col2 = st.columns(2)
        
        with col1:
            submitted = st.form_submit_button("💾 保存配置", type="primary")
        
        with col2:
            restart_monitoring = st.form_submit_button("🔄 重启监控")
        
        if submitted:
            update_monitoring_config(
                heartbeat_interval, reconnection_interval, 
                cleanup_interval_hours * 3600, max_queue_size,
                max_history_hours, http_timeout
            )
        
        if restart_monitoring:
            restart_monitoring_tasks()

def show_detailed_stats():
    """显示详细统计"""
    st.subheader("📋 详细系统统计")
    
    # 获取统计数据
    api_client = st.session_state.api_client
    stats_response = api_client.get_stats()
    
    if not stats_response:
        show_error_message("无法获取统计数据")
        return
    
    stats_data = stats_response.get('data', {})
    
    # 显示原始统计数据
    with st.expander("📊 原始统计数据", expanded=True):
        st.code(format_json(stats_data), language='json')
    
    # 服务健康检查结果
    health_response = api_client.get_health()
    
    if health_response:
        health_data = health_response.get('data', {})
        
        st.markdown("#### 🏥 服务健康检查")
        
        if 'services' in health_data:
            services_health = health_data['services']
            
            for service_name, status in services_health.items():
                status_icon = "🟢" if status == "healthy" else "🔴"
                st.write(f"{status_icon} {service_name}: {status}")
        
        # 健康检查时间戳
        if 'timestamp' in health_data:
            st.caption(f"检查时间: {health_data['timestamp']}")

def update_monitoring_config(heartbeat_interval: int, reconnection_interval: int,
                           cleanup_interval: int, max_queue_size: int,
                           max_history_hours: int, http_timeout: int):
    """更新监控配置"""
    api_client = st.session_state.api_client

    config = {
        "heartbeat_interval_seconds": heartbeat_interval,
        "reconnection_interval_seconds": reconnection_interval,
        "cleanup_interval_hours": cleanup_interval // 3600,  # 转换为小时
        "max_reconnection_queue_size": max_queue_size,
        "max_heartbeat_history_hours": max_history_hours,
        "http_timeout_seconds": http_timeout
    }

    with st.spinner("更新监控配置..."):
        response = api_client.update_monitoring_config(config)

        if response and response.get('success'):
            show_success_message("监控配置更新成功")
            updated_fields = response.get('data', {}).get('updated_fields', [])
            if updated_fields:
                st.info(f"已更新的配置项: {', '.join(updated_fields)}")
            st.rerun()
        else:
            error_msg = response.get('message', '未知错误') if response else '请求失败'
            show_error_message(f"监控配置更新失败: {error_msg}")

def restart_monitoring_tasks():
    """重启监控任务"""
    api_client = st.session_state.api_client
    
    with st.spinner("重启监控任务..."):
        response = api_client.restart_monitoring()
        
        if response and response.get('success'):
            show_success_message("监控任务重启成功")
            st.rerun()
        else:
            show_error_message("监控任务重启失败")
