"""
API功能展示页面
展示所有新添加的API接口功能
"""

import streamlit as st
from typing import Dict, List
import json

from utils.helpers import (
    show_success_message, show_error_message, show_info_message, show_warning_message,
    format_json
)

def show():
    """显示API功能展示页面"""
    st.header("🚀 API功能展示")
    st.markdown("展示MCPStore Web项目中所有可用的API接口功能")
    
    # 创建标签页
    tab1, tab2, tab3, tab4 = st.tabs(["🛠️ 服务管理", "📊 监控管理", "👥 Agent管理", "🧪 API测试"])
    
    with tab1:
        show_service_management_apis()
    
    with tab2:
        show_monitoring_apis()
    
    with tab3:
        show_agent_management_apis()
    
    with tab4:
        show_api_testing()

def show_service_management_apis():
    """展示服务管理API"""
    st.subheader("🛠️ 服务管理API功能")
    
    # API状态检查
    api_client = st.session_state.api_client
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("#### ✅ 已实现的API")
        implemented_apis = [
            "📋 list_services - 获取服务列表",
            "➕ add_service - 添加服务",
            "🔍 check_services - 健康检查",
            "📊 get_service_info - 获取服务详情",
            "🗑️ delete_service - 删除服务",
            "✏️ update_service - 更新服务配置",
            "🔄 restart_service - 重启服务",
            "📦 batch_add_services - 批量添加服务"
        ]
        
        for api in implemented_apis:
            st.write(f"• {api}")
    
    with col2:
        st.markdown("#### 🧪 API测试")
        
        if st.button("测试获取服务列表", key="test_list_services"):
            test_list_services()
        
        if st.button("测试健康检查", key="test_check_services"):
            test_check_services()
        
        if st.button("测试系统健康状态", key="test_health"):
            test_system_health()

def show_monitoring_apis():
    """展示监控管理API"""
    st.subheader("📊 监控管理API功能")
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("#### ✅ 已实现的API")
        monitoring_apis = [
            "📈 get_monitoring_status - 获取监控状态",
            "⚙️ update_monitoring_config - 更新监控配置",
            "🔄 restart_monitoring - 重启监控任务",
            "🏥 get_health - 系统健康检查",
            "📊 get_stats - 获取统计信息"
        ]
        
        for api in monitoring_apis:
            st.write(f"• {api}")
    
    with col2:
        st.markdown("#### 🧪 API测试")
        
        if st.button("测试监控状态", key="test_monitoring_status"):
            test_monitoring_status()
        
        if st.button("测试系统统计", key="test_system_stats"):
            test_system_stats()

def show_agent_management_apis():
    """展示Agent管理API"""
    st.subheader("👥 Agent管理API功能")
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("#### ✅ 已实现的API")
        agent_apis = [
            "📋 list_agent_services - 获取Agent服务列表",
            "➕ add_agent_service - 为Agent添加服务",
            "🔧 list_agent_tools - 获取Agent工具列表",
            "🗑️ delete_agent_service - 删除Agent服务",
            "🔄 reset_agent_config - 重置Agent配置",
            "📊 get_agent_stats - 获取Agent统计信息"
        ]
        
        for api in agent_apis:
            st.write(f"• {api}")
    
    with col2:
        st.markdown("#### 🧪 Agent测试")
        
        test_agent_id = st.text_input(
            "测试Agent ID",
            value="test_agent_001",
            help="输入要测试的Agent ID"
        )
        
        if st.button("测试Agent服务列表", key="test_agent_services"):
            test_agent_services(test_agent_id)
        
        if st.button("测试Agent工具列表", key="test_agent_tools"):
            test_agent_tools(test_agent_id)
        
        if st.button("测试Agent统计信息", key="test_agent_stats"):
            test_agent_stats(test_agent_id)

def show_api_testing():
    """显示API测试工具"""
    st.subheader("🧪 API测试工具")
    
    # API连接测试
    st.markdown("#### 🔗 连接测试")
    
    col1, col2, col3 = st.columns(3)
    
    with col1:
        if st.button("测试API连接", key="test_connection"):
            test_api_connection()
    
    with col2:
        if st.button("测试所有基础API", key="test_all_basic"):
            test_all_basic_apis()
    
    with col3:
        if st.button("生成API报告", key="generate_report"):
            generate_api_report()

# ==================== 测试函数 ====================

def test_list_services():
    """测试获取服务列表"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试获取服务列表..."):
        response = api_client.list_services()
        
        if response:
            services = response.get('data', [])
            show_success_message(f"✅ 获取服务列表成功，共 {len(services)} 个服务")
            
            if services:
                with st.expander("📋 服务列表详情"):
                    for i, service in enumerate(services[:5]):  # 只显示前5个
                        st.write(f"{i+1}. {service.get('name', 'Unknown')} - {service.get('status', 'Unknown')}")
                    if len(services) > 5:
                        st.write(f"... 还有 {len(services) - 5} 个服务")
        else:
            show_error_message("❌ 获取服务列表失败")

def test_check_services():
    """测试健康检查"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试健康检查..."):
        response = api_client.check_services()
        
        if response:
            show_success_message("✅ 健康检查完成")
            
            with st.expander("🏥 健康检查结果"):
                st.code(format_json(response), language='json')
        else:
            show_error_message("❌ 健康检查失败")

def test_system_health():
    """测试系统健康状态"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试系统健康状态..."):
        response = api_client.get_health()
        
        if response:
            health_data = response.get('data', {})
            status = health_data.get('status', 'unknown')
            
            if status == 'healthy':
                show_success_message(f"✅ 系统状态: {status}")
            elif status == 'degraded':
                show_warning_message(f"⚠️ 系统状态: {status}")
            else:
                show_error_message(f"❌ 系统状态: {status}")
            
            with st.expander("🏥 系统健康详情"):
                st.code(format_json(health_data), language='json')
        else:
            show_error_message("❌ 获取系统健康状态失败")

def test_monitoring_status():
    """测试监控状态"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试监控状态..."):
        response = api_client.get_monitoring_status()
        
        if response:
            monitoring_data = response.get('data', {})
            show_success_message("✅ 监控状态获取成功")
            
            with st.expander("📊 监控状态详情"):
                # 显示监控任务状态
                tasks = monitoring_data.get('monitoring_tasks', {})
                st.markdown("**监控任务状态:**")
                for task, status in tasks.items():
                    if isinstance(status, bool):
                        icon = "🟢" if status else "🔴"
                        st.write(f"• {task}: {icon} {'运行中' if status else '已停止'}")
                
                # 显示服务统计
                stats = monitoring_data.get('service_statistics', {})
                if stats:
                    st.markdown("**服务统计:**")
                    st.write(f"• 总服务数: {stats.get('total_services', 0)}")
                    st.write(f"• 健康服务: {stats.get('healthy_services', 0)}")
                    st.write(f"• 健康率: {stats.get('health_percentage', 0)}%")
        else:
            show_error_message("❌ 获取监控状态失败")

def test_system_stats():
    """测试系统统计"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试系统统计..."):
        response = api_client.get_stats()
        
        if response:
            stats_data = response.get('data', {})
            show_success_message("✅ 系统统计获取成功")
            
            with st.expander("📊 系统统计详情"):
                st.code(format_json(stats_data), language='json')
        else:
            show_error_message("❌ 获取系统统计失败")

def test_agent_services(agent_id: str):
    """测试Agent服务列表"""
    if not agent_id:
        show_error_message("请输入Agent ID")
        return
    
    api_client = st.session_state.api_client
    
    with st.spinner(f"测试Agent {agent_id} 服务列表..."):
        response = api_client.list_agent_services(agent_id)
        
        if response:
            services = response.get('data', [])
            show_success_message(f"✅ Agent {agent_id} 服务列表获取成功，共 {len(services)} 个服务")
        else:
            show_warning_message(f"⚠️ Agent {agent_id} 服务列表获取失败（可能Agent不存在）")

def test_agent_tools(agent_id: str):
    """测试Agent工具列表"""
    if not agent_id:
        show_error_message("请输入Agent ID")
        return
    
    api_client = st.session_state.api_client
    
    with st.spinner(f"测试Agent {agent_id} 工具列表..."):
        response = api_client.list_agent_tools(agent_id)
        
        if response:
            tools = response.get('data', [])
            show_success_message(f"✅ Agent {agent_id} 工具列表获取成功，共 {len(tools)} 个工具")
        else:
            show_warning_message(f"⚠️ Agent {agent_id} 工具列表获取失败（可能Agent不存在）")

def test_agent_stats(agent_id: str):
    """测试Agent统计信息"""
    if not agent_id:
        show_error_message("请输入Agent ID")
        return
    
    api_client = st.session_state.api_client
    
    with st.spinner(f"测试Agent {agent_id} 统计信息..."):
        response = api_client.get_agent_stats(agent_id)
        
        if response:
            stats_data = response.get('data', {})
            show_success_message(f"✅ Agent {agent_id} 统计信息获取成功")
            
            with st.expander(f"📊 Agent {agent_id} 统计详情"):
                st.code(format_json(stats_data), language='json')
        else:
            show_warning_message(f"⚠️ Agent {agent_id} 统计信息获取失败（可能Agent不存在）")

def test_api_connection():
    """测试API连接"""
    api_client = st.session_state.api_client
    
    with st.spinner("测试API连接..."):
        if api_client.backend.test_connection():
            show_success_message("✅ API连接正常")
        else:
            show_error_message("❌ API连接失败")

def test_all_basic_apis():
    """测试所有基础API"""
    st.info("🧪 开始测试所有基础API...")
    
    # 依次测试各个API
    test_api_connection()
    test_list_services()
    test_check_services()
    test_system_health()
    test_monitoring_status()
    test_system_stats()
    
    show_success_message("✅ 所有基础API测试完成")

def generate_api_report():
    """生成API报告"""
    api_client = st.session_state.api_client
    
    with st.spinner("生成API报告..."):
        report = {
            "api_connection": api_client.backend.test_connection(),
            "services_count": 0,
            "tools_count": 0,
            "monitoring_status": "unknown",
            "system_health": "unknown"
        }
        
        # 获取服务数量
        services_response = api_client.list_services()
        if services_response:
            report["services_count"] = len(services_response.get('data', []))
        
        # 获取工具数量
        tools_response = api_client.list_tools()
        if tools_response:
            report["tools_count"] = len(tools_response.get('data', []))
        
        # 获取监控状态
        monitoring_response = api_client.get_monitoring_status()
        if monitoring_response:
            tasks = monitoring_response.get('data', {}).get('monitoring_tasks', {})
            active_tasks = sum(1 for status in tasks.values() if isinstance(status, bool) and status)
            report["monitoring_status"] = f"{active_tasks} 个任务运行中"
        
        # 获取系统健康状态
        health_response = api_client.get_health()
        if health_response:
            report["system_health"] = health_response.get('data', {}).get('status', 'unknown')
        
        show_success_message("✅ API报告生成完成")
        
        with st.expander("📊 API状态报告", expanded=True):
            col1, col2, col3, col4 = st.columns(4)
            
            with col1:
                st.metric("API连接", "✅ 正常" if report["api_connection"] else "❌ 异常")
            
            with col2:
                st.metric("服务数量", report["services_count"])
            
            with col3:
                st.metric("工具数量", report["tools_count"])
            
            with col4:
                st.metric("系统健康", report["system_health"])
            
            st.markdown("**监控状态:**")
            st.write(f"• {report['monitoring_status']}")
