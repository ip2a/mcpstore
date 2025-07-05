#!/usr/bin/env python3
"""
MCPStore Web管理界面
基于Streamlit的可视化管理平台

作者: MCPStore团队
版本: v2.0.0 - 增强版
"""

import streamlit as st
import requests
from datetime import datetime
import json
import time
from typing import Dict, List, Optional, Any

# 配置页面
st.set_page_config(
    page_title="MCPStore 管理面板",
    page_icon="🚀",
    layout="wide",
    initial_sidebar_state="expanded"
)

# 导入增强模块
from utils.api_client import MCPStoreAPI
from utils.config_manager import SessionManager, WebConfigManager
from utils.store_manager import initialize_store, is_store_initialized
from components.ui_components import (
    StatusIndicator, MetricCard, NotificationSystem,
    QuickActions, LoadingSpinner
)
from style import apply_custom_styles

# 导入页面模块
from pages import (
    service_management,
    tool_management,
    agent_management,
    monitoring,
    configuration,
    api_showcase
)

def main():
    """主应用函数"""

    # 应用自定义样式
    apply_custom_styles()

    # 清理可能的加载状态
    if 'page_loading' in st.session_state:
        del st.session_state.page_loading

    # 初始化MCPStore实例（重构后的核心变更）
    if not is_store_initialized():
        try:
            with st.spinner("初始化MCPStore..."):
                initialize_store()
            st.success("✅ MCPStore初始化成功")
        except Exception as e:
            st.error(f"❌ MCPStore初始化失败: {e}")
            st.stop()

    # 初始化增强会话状态
    SessionManager.init_session_state()

    # 初始化API客户端（重构后使用直接调用）
    if 'api_client' not in st.session_state:
        # 重构后：默认使用直接调用模式，不再依赖HTTP API
        backend_type = st.session_state.get('api_backend_type', 'direct')
        base_url = st.session_state.get('api_base_url', 'direct')
        st.session_state.api_client = MCPStoreAPI()

    # 显示通知
    NotificationSystem.show_notifications()

    # 页面标题和状态
    render_header()

    # 侧边栏导航
    with st.sidebar:
        render_sidebar()

    # 主内容区域
    render_main_content()

    # 处理全局模态窗口
    handle_global_modals()

def handle_global_modals():
    """处理全局模态窗口"""
    from components.modal_components import ServiceModal, ToolModal, InfoModal

    # 服务详情模态窗口
    if st.session_state.get('show_service_detail_modal', False):
        selected_service = st.session_state.get('selected_service_detail')
        if selected_service:
            with st.container():
                ServiceModal.show_service_details(selected_service)
                if st.button("❌ 关闭详情"):
                    st.session_state.show_service_detail_modal = False
                    st.rerun()

    # 系统信息模态窗口
    if st.session_state.get('show_system_info_modal', False):
        InfoModal.show_system_info()

def render_header():
    """渲染页面头部 - 仅状态栏"""

    # 只保留状态栏
    render_status_bar()

def render_status_bar():
    """渲染轻量化状态栏"""
    # 轻量化状态栏 - 固定在右上角
    if 'api_client' in st.session_state:
        backend_info = st.session_state.api_client.get_backend_info()
        status = "healthy" if backend_info.get('status') else "disconnected"

        import datetime
        current_time = datetime.datetime.now().strftime("%H:%M")

        status_icon = "🟢" if status == "healthy" else "🔴"
        status_color = "#28a745" if status == "healthy" else "#dc3545"

        # 轻量化状态显示
        status_html = f"""
        <div style="
            position: fixed;
            top: 1rem;
            right: 1rem;
            z-index: 1000;
            background: rgba(255, 255, 255, 0.95);
            backdrop-filter: blur(10px);
            border: 1px solid #e9ecef;
            border-radius: 20px;
            padding: 0.4rem 0.8rem;
            font-size: 0.75rem;
            color: #6c757d;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            display: flex;
            align-items: center;
            gap: 0.5rem;
        ">
            <span style="color: {status_color};">{status_icon}</span>
            <span>{current_time}</span>
            <span style="cursor: pointer;" onclick="location.reload()">🔄</span>
        </div>
        """

        st.markdown(status_html, unsafe_allow_html=True)

def render_sidebar():
    """渲染侧边栏"""

    # 品牌标识区域
    render_brand_section()

    # 专业分隔线
    st.markdown("""
    <div style="
        height: 1px;
        background: linear-gradient(90deg, transparent, #dee2e6, transparent);
        margin: 1rem 0;
    "></div>
    """, unsafe_allow_html=True)

    # 主导航菜单
    render_navigation_menu()

    # 专业分隔线
    st.markdown("""
    <div style="
        height: 1px;
        background: linear-gradient(90deg, transparent, #dee2e6, transparent);
        margin: 1.5rem 0 1rem 0;
    "></div>
    """, unsafe_allow_html=True)

    # 系统状态
    render_system_status()

    # 专业分隔线
    st.markdown("""
    <div style="
        height: 1px;
        background: linear-gradient(90deg, transparent, #dee2e6, transparent);
        margin: 1.5rem 0 1rem 0;
    "></div>
    """, unsafe_allow_html=True)

    # 后端配置
    render_backend_config()

def render_brand_section():
    """渲染品牌标识区域"""
    st.markdown("""
    <div style="padding: 1.5rem 1rem 1rem 1rem;">
        <div style="
            font-size: 1.2rem;
            font-weight: 600;
            color: #2c3e50;
            margin-bottom: 0.25rem;
            letter-spacing: -0.5px;
        ">
            MCPStore
        </div>
        <div style="
            font-size: 0.75rem;
            color: #7f8c8d;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            font-weight: 500;
        ">
            Management Console
        </div>
    </div>
    """, unsafe_allow_html=True)

def render_navigation_menu():
    """渲染导航菜单"""

    # 主导航菜单 - 专业设计
    page_options = [
        ("overview", "Overview", "📊"),
        ("service_management", "Services", "🔧"),
        ("tool_management", "Tools", "⚙️"),
        ("agent_management", "Agents", "👤"),
        ("monitoring", "Monitor", "📈"),
        ("configuration", "Settings", "⚙️"),
        ("api_showcase", "API Demo", "🚀")
    ]

    # 获取当前选中的页面
    current_page = st.session_state.get('current_page', 'overview')

    # 导航菜单标题
    st.markdown("""
    <div style="
        padding: 0 0 0.5rem 0;
        margin-bottom: 0.5rem;
    ">
        <div style="
            font-size: 0.7rem;
            font-weight: 600;
            color: #adb5bd;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        ">
            Navigation
        </div>
    </div>
    """, unsafe_allow_html=True)

    # 使用简单的按钮导航
    for page_key, name, icon in page_options:
        is_current = current_page == page_key

        # 使用原生按钮，通过CSS样式化
        button_type = "primary" if is_current else "secondary"

        if st.button(
            f"{icon}  {name}",
            key=f"nav_{page_key}",
            use_container_width=True,
            type=button_type,
            help=f"切换到{name}页面"
        ):
            # 直接切换页面，不使用加载状态
            st.session_state.current_page = page_key
            st.rerun()

def render_system_status():
    """渲染系统状态"""

    # 简洁的状态标题
    st.markdown("""
    <div style="
        padding: 0 0 0.5rem 0;
        margin-bottom: 0.5rem;
    ">
        <div style="
            font-size: 0.7rem;
            font-weight: 600;
            color: #adb5bd;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        ">
            System Status
        </div>
    </div>
    """, unsafe_allow_html=True)

    # 获取系统数据
    try:
        service_data = get_cached_service_data()
        agent_count = len(st.session_state.get('agents', []))

        # Store状态
        total_services = service_data.get('count', 0)
        healthy_services = service_data.get('healthy', 0)
        store_health_rate = int((healthy_services / total_services * 100)) if total_services > 0 else 100
        store_status_color = "#4CAF50" if store_health_rate > 80 else "#FF9800" if store_health_rate > 50 else "#F44336"
        store_status_text = "正常" if store_health_rate > 80 else "警告" if store_health_rate > 50 else "异常"

        # Agent状态 (简化处理)
        agent_status_color = "#4CAF50" if agent_count > 0 else "#6c757d"
        agent_status_text = "活跃" if agent_count > 0 else "无"

        status_html = f"""
        <!-- Store状态 -->
        <div style="
            background: #f8f9fa;
            border-left: 3px solid {store_status_color};
            border-radius: 4px;
            padding: 0.75rem;
            margin-bottom: 0.5rem;
        ">
            <div style="
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 0.25rem;
            ">
                <span style="
                    font-size: 0.75rem;
                    font-weight: 600;
                    color: #495057;
                ">Store</span>
                <span style="
                    font-size: 0.7rem;
                    padding: 0.15rem 0.4rem;
                    border-radius: 8px;
                    background: {store_status_color};
                    color: white;
                    font-weight: 500;
                ">{store_status_text}</span>
            </div>
            <div style="
                font-size: 0.7rem;
                color: #6c757d;
            ">
                {total_services} services • {healthy_services} healthy
            </div>
        </div>

        <!-- Agent状态 -->
        <div style="
            background: #f8f9fa;
            border-left: 3px solid {agent_status_color};
            border-radius: 4px;
            padding: 0.75rem;
            margin-bottom: 0.5rem;
        ">
            <div style="
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 0.25rem;
            ">
                <span style="
                    font-size: 0.75rem;
                    font-weight: 600;
                    color: #495057;
                ">Agents</span>
                <span style="
                    font-size: 0.7rem;
                    padding: 0.15rem 0.4rem;
                    border-radius: 8px;
                    background: {agent_status_color};
                    color: white;
                    font-weight: 500;
                ">{agent_status_text}</span>
            </div>
            <div style="
                font-size: 0.7rem;
                color: #6c757d;
            ">
                {agent_count} total
            </div>
        </div>
        """

        st.markdown(status_html, unsafe_allow_html=True)

    except Exception as e:
        st.markdown(f"""
        <div style="
            background: #f8d7da;
            border: 1px solid #f5c6cb;
            border-radius: 8px;
            padding: 0.75rem;
            text-align: center;
        ">
            <span style="font-size: 0.8rem; color: #721c24;">状态获取失败</span>
        </div>
        """, unsafe_allow_html=True)

def render_backend_config():
    """渲染后端配置"""

    # 简洁的配置标题
    st.markdown("""
    <div style="
        padding: 0 0 0.5rem 0;
        margin-bottom: 0.5rem;
    ">
        <div style="
            font-size: 0.7rem;
            font-weight: 600;
            color: #adb5bd;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        ">
            Backend Config
        </div>
    </div>
    """, unsafe_allow_html=True)

    config_manager = st.session_state.config_manager

    # 后端类型选择 - 重构后默认为直接调用
    backend_type = st.selectbox(
        "Type",
        ["direct", "http"],
        index=0 if st.session_state.get('api_backend_type', 'direct') == "direct" else 1,
        help="Direct: 直接调用MCPStore方法 | HTTP: API调用（已弃用）",
        label_visibility="collapsed"
    )

    # API服务器地址（仅HTTP后端）
    if backend_type == "http":
        api_base = st.text_input(
            "API Server",
            value=st.session_state.api_base_url,
            help="MCPStore API server address",
            placeholder="http://localhost:8000",
            label_visibility="collapsed"
        )

        # 更新配置
        if api_base != st.session_state.api_base_url:
            st.session_state.api_base_url = api_base
            config_manager.set('api.base_url', api_base)
    else:
        api_base = None
        st.markdown("""
        <div style="
            background: #d4edda;
            border-left: 3px solid #28a745;
            padding: 0.5rem;
            border-radius: 4px;
            font-size: 0.75rem;
            color: #155724;
        ">
            ✅ 直接调用模式 - 无需API服务器
        </div>
        """, unsafe_allow_html=True)

    # 后端切换
    if backend_type != st.session_state.api_backend_type:
        st.session_state.api_backend_type = backend_type
        config_manager.set('api.backend_type', backend_type)

        # 重新初始化API客户端（重构后）
        if backend_type == "direct":
            st.session_state.api_client = MCPStoreAPI()
        else:
            st.session_state.api_client = MCPStoreAPI(backend_type, api_base)
        SessionManager.add_operation_history(f"切换后端到: {backend_type}")
        st.rerun()

    # 连接测试 - 简化按钮
    if st.button("Test Connection", key="backend_test_connection", use_container_width=True, type="secondary"):
        test_connection()

def test_connection():
    """测试连接"""
    with st.spinner("检查连接..."):
        if st.session_state.api_client.test_connection():
            SessionManager.add_notification("连接成功！", "success")
            SessionManager.add_operation_history("连接测试", {"result": "success"})
        else:
            SessionManager.add_notification("连接失败！请检查配置", "error")
            SessionManager.add_operation_history("连接测试", {"result": "failed"})

def render_quick_actions():
    """渲染快速操作"""

    # 添加服务按钮
    if st.button("➕ 添加服务", use_container_width=True, help="快速添加新服务", key="sidebar_add_service"):
        st.session_state.show_add_service_modal = True

    # 测试工具按钮
    if st.button("🧪 测试工具", use_container_width=True, help="快速测试工具", key="sidebar_test_tool"):
        st.session_state.show_test_tool_modal = True

    # 系统状态按钮
    if st.button("📊 系统状态", use_container_width=True, help="查看系统状态", key="sidebar_system_status"):
        st.session_state.show_system_status_modal = True

    # 清除缓存按钮
    if st.button("🗑️ 清除缓存", use_container_width=True, help="清除所有缓存数据", key="sidebar_clear_cache"):
        SessionManager.clear_cache()
        SessionManager.add_notification("缓存已清除", "success")
        st.rerun()

    # 处理模态窗口
    handle_modals()

def handle_modals():
    """处理模态窗口"""

    # 添加服务模态窗口
    if st.session_state.get('show_add_service_modal', False):
        show_add_service_modal()

    # 测试工具模态窗口
    if st.session_state.get('show_test_tool_modal', False):
        show_test_tool_modal()

    # 系统状态模态窗口
    if st.session_state.get('show_system_status_modal', False):
        show_system_status_modal()

@st.dialog("➕ 快速添加服务")
def show_add_service_modal():
    """显示添加服务模态窗口"""
    st.markdown("### 选择添加方式")

    # 预设服务
    config_manager = st.session_state.config_manager
    preset_services = config_manager.get_preset_services()

    if preset_services:
        st.markdown("#### 🎯 预设服务")
        for preset in preset_services:
            col1, col2 = st.columns([3, 1])
            with col1:
                st.write(f"**{preset['name']}**")
                st.caption(preset['description'])
            with col2:
                if st.button(f"添加", key=f"add_preset_{preset['name']}"):
                    add_preset_service_quick(preset)
                    st.session_state.show_add_service_modal = False
                    st.rerun()

    st.markdown("#### 🔧 自定义服务")

    with st.form("quick_add_service"):
        name = st.text_input("服务名称", placeholder="输入服务名称")
        url = st.text_input("服务URL", placeholder="http://example.com/mcp")

        col1, col2 = st.columns(2)
        with col1:
            if st.form_submit_button("✅ 添加服务", type="primary"):
                if name and url:
                    add_custom_service_quick(name, url)
                    st.session_state.show_add_service_modal = False
                    st.rerun()
                else:
                    st.error("请填写服务名称和URL")

        with col2:
            if st.form_submit_button("❌ 取消"):
                st.session_state.show_add_service_modal = False
                st.rerun()

@st.dialog("🧪 快速测试工具")
def show_test_tool_modal():
    """显示测试工具模态窗口"""
    st.markdown("### 选择要测试的工具")

    # 获取工具列表
    try:
        response = st.session_state.api_client.list_tools()
        if response and 'data' in response:
            tools = response['data']

            if tools:
                tool_names = [f"{tool.get('name')} ({tool.get('service_name')})" for tool in tools]
                selected_tool_name = st.selectbox("选择工具", tool_names)

                if selected_tool_name:
                    # 找到选中的工具
                    selected_tool = None
                    for tool in tools:
                        if f"{tool.get('name')} ({tool.get('service_name')})" == selected_tool_name:
                            selected_tool = tool
                            break

                    if selected_tool:
                        st.markdown(f"**工具**: {selected_tool.get('name')}")
                        st.markdown(f"**服务**: {selected_tool.get('service_name')}")
                        st.markdown(f"**描述**: {selected_tool.get('description', '无描述')}")

                        col1, col2 = st.columns(2)
                        with col1:
                            if st.button("🧪 测试此工具", type="primary"):
                                st.session_state.selected_tool_for_test = selected_tool
                                st.session_state.show_test_tool_modal = False
                                st.session_state.switch_to_tool_tab = True
                                st.rerun()

                        with col2:
                            if st.button("❌ 取消"):
                                st.session_state.show_test_tool_modal = False
                                st.rerun()
            else:
                st.info("暂无可用工具")
                if st.button("❌ 关闭"):
                    st.session_state.show_test_tool_modal = False
                    st.rerun()
        else:
            st.error("无法获取工具列表")
            if st.button("❌ 关闭"):
                st.session_state.show_test_tool_modal = False
                st.rerun()
    except Exception as e:
        st.error(f"获取工具列表失败: {e}")
        if st.button("❌ 关闭"):
            st.session_state.show_test_tool_modal = False
            st.rerun()

@st.dialog("📊 系统状态")
def show_system_status_modal():
    """显示系统状态模态窗口"""
    st.markdown("### 实时系统状态")

    # 获取系统数据
    service_data = get_cached_service_data()
    tool_data = get_cached_tool_data()

    # 状态指标
    col1, col2, col3 = st.columns(3)

    with col1:
        st.metric("服务总数", service_data.get('count', 0))

    with col2:
        st.metric("健康服务", service_data.get('healthy', 0))

    with col3:
        health_percentage = calculate_system_health(service_data)
        st.metric("健康率", f"{health_percentage}%")

    # 服务列表
    services = service_data.get('services', [])
    if services:
        st.markdown("#### 服务状态")
        for service in services[:5]:  # 只显示前5个
            status = service.get('status', 'unknown')
            status_icon = "🟢" if status == 'healthy' else "🔴" if status == 'unhealthy' else "🟡"
            st.write(f"{status_icon} {service.get('name', 'Unknown')}")

    # 关闭按钮
    if st.button("❌ 关闭", use_container_width=True):
        st.session_state.show_system_status_modal = False
        st.rerun()

def add_preset_service_quick(preset):
    """快速添加预设服务"""
    try:
        api_client = st.session_state.api_client
        response = api_client.add_service(preset)

        if response and response.get('success'):
            SessionManager.add_notification(f"服务 {preset['name']} 添加成功！", "success")
            SessionManager.add_operation_history(f"快速添加预设服务: {preset['name']}")
            SessionManager.clear_cache()  # 清除缓存以刷新数据
        else:
            SessionManager.add_notification(f"服务 {preset['name']} 添加失败", "error")
    except Exception as e:
        SessionManager.add_notification(f"添加服务时出错: {e}", "error")

def add_custom_service_quick(name, url):
    """快速添加自定义服务"""
    try:
        config = {"name": name, "url": url}
        api_client = st.session_state.api_client
        response = api_client.add_service(config)

        if response and response.get('success'):
            SessionManager.add_notification(f"服务 {name} 添加成功！", "success")
            SessionManager.add_operation_history(f"快速添加自定义服务: {name}")
            SessionManager.clear_cache()  # 清除缓存以刷新数据
        else:
            SessionManager.add_notification(f"服务 {name} 添加失败", "error")
    except Exception as e:
        SessionManager.add_notification(f"添加服务时出错: {e}", "error")

def render_system_info():
    """渲染系统信息"""
    st.subheader("📊 系统信息")

    # 缓存统计
    cache_count = len(st.session_state.data_cache)
    st.metric("缓存项", cache_count)

    # 操作历史
    history_count = len(st.session_state.operation_history)
    st.metric("操作历史", history_count)

    # 最后刷新时间
    if 'last_refresh' in st.session_state:
        st.caption(f"最后刷新: {st.session_state.last_refresh.strftime('%H:%M:%S')}")

    # 配置信息
    with st.expander("🔧 配置信息"):
        config_manager = st.session_state.config_manager
        st.json({
            "后端类型": st.session_state.api_backend_type,
            "API地址": st.session_state.api_base_url,
            "自动刷新": config_manager.get('ui.auto_refresh'),
            "刷新间隔": config_manager.get('ui.refresh_interval')
        })

def render_main_content():
    """渲染主内容区域 - 根据侧边栏选择显示内容"""

    # 获取当前选中的页面
    current_page = st.session_state.get('current_page', 'overview')

    # 清除加载状态（如果存在）
    if st.session_state.get('page_loading', False):
        st.session_state.page_loading = False

    # 根据选择显示对应页面
    try:
        if current_page == 'overview':
            show_enhanced_system_overview()
        elif current_page == 'service_management':
            service_management.show()
        elif current_page == 'tool_management':
            tool_management.show()
        elif current_page == 'agent_management':
            agent_management.show()
        elif current_page == 'monitoring':
            monitoring.show()
        elif current_page == 'configuration':
            configuration.show()
        elif current_page == 'api_showcase':
            api_showcase.show()
        else:
            # 默认显示系统概览
            show_enhanced_system_overview()
    except Exception as e:
        st.error(f"页面加载失败: {e}")
        st.info("请尝试刷新页面或联系管理员")

def show_loading_screen():
    """显示简单加载提示"""
    st.info("页面加载中，请稍候...")

    # 清除加载状态
    st.session_state.page_loading = False

def show_enhanced_system_overview():
    """显示增强的系统概览"""

    # 欢迎信息
    st.markdown("## 🏠 欢迎使用 MCPStore 管理面板")
    st.markdown("这里是您的MCP服务管理中心，可以监控和管理所有MCP服务。")

    # 使用缓存获取数据
    service_data = get_cached_service_data()
    tool_data = get_cached_tool_data()

    # 系统状态卡片
    st.markdown("### 📊 系统状态")

    col1, col2, col3, col4 = st.columns(4)

    with col1:
        total_services = service_data.get('count', 0)
        healthy_services = service_data.get('healthy', 0)
        st.metric(
            label="🛠️ 服务总数",
            value=total_services,
            delta=f"健康: {healthy_services}",
            help="已注册的MCP服务数量"
        )

    with col2:
        total_tools = tool_data.get('count', 0)
        st.metric(
            label="🔧 工具总数",
            value=total_tools,
            help="所有服务提供的工具数量"
        )

    with col3:
        agent_count = len(st.session_state.get('agents', []))
        st.metric(
            label="👥 Agent数量",
            value=agent_count,
            help="已创建的Agent数量"
        )

    with col4:
        health_percentage = calculate_system_health(service_data)
        delta_color = "normal" if health_percentage > 80 else "inverse"
        st.metric(
            label="💚 系统健康度",
            value=f"{health_percentage}%",
            delta="良好" if health_percentage > 80 else "需要关注",
            delta_color=delta_color,
            help="服务健康状态比例"
        )
    
    st.markdown("---")

    # 服务概览和活动
    col1, col2 = st.columns([2, 1])

    with col1:
        st.markdown("### 📈 服务概览")
        show_service_overview_table(service_data)

    with col2:
        st.markdown("### 🔔 最近活动")
        show_recent_activities()

    # 快速操作面板
    st.markdown("---")
    st.markdown("### ⚡ 快速操作")
    st.markdown("点击下方按钮快速执行常用操作：")

    col1, col2, col3, col4 = st.columns(4)

    with col1:
        if st.button("➕ 添加服务", use_container_width=True, help="快速添加新的MCP服务", key="overview_add_service"):
            st.session_state.show_add_service_modal = True
            st.rerun()

    with col2:
        if st.button("🧪 测试工具", use_container_width=True, help="测试可用的MCP工具", key="overview_test_tool"):
            st.session_state.show_test_tool_modal = True
            st.rerun()

    with col3:
        if st.button("👤 创建Agent", use_container_width=True, help="创建新的Agent", key="overview_create_agent"):
            st.session_state.current_page = "agent_management"
            st.rerun()

    with col4:
        if st.button("📊 详细监控", use_container_width=True, help="查看详细的系统监控", key="overview_monitoring"):
            st.session_state.current_page = "monitoring"
            st.rerun()

def show_service_overview_table(service_data):
    """显示服务概览表格"""
    services = service_data.get('services', [])

    if not services:
        st.info("暂无已注册的服务")
        return

    # 显示前5个服务的状态
    st.markdown("**服务状态概览** (显示前5个)")

    for i, service in enumerate(services[:5]):
        col1, col2, col3 = st.columns([2, 1, 1])

        with col1:
            status = service.get('status', 'unknown')
            status_icon = "🟢" if status == 'healthy' else "🔴" if status == 'unhealthy' else "🟡"
            st.write(f"{status_icon} **{service.get('name', 'Unknown')}**")

        with col2:
            tool_count = service.get('tool_count', 0)
            st.write(f"🔧 {tool_count} 工具")

        with col3:
            if st.button("详情", key=f"overview_service_detail_{i}_{service.get('name', 'unknown')}", help=f"查看 {service.get('name')} 的详情"):
                st.session_state.selected_service_detail = service
                st.session_state.show_service_detail_modal = True
                st.rerun()

    if len(services) > 5:
        st.caption(f"还有 {len(services) - 5} 个服务，请到服务管理页面查看全部")

# 这些函数已经被新的缓存函数替代，保留作为备用
def get_service_count():
    """获取服务数量（备用函数）"""
    try:
        response = st.session_state.api_client.list_services()
        if response and 'data' in response:
            return len(response['data'])
        return 0
    except:
        return "N/A"

def get_tool_count():
    """获取工具数量（备用函数）"""
    try:
        response = st.session_state.api_client.list_tools()
        if response and 'data' in response:
            return len(response['data'])
        return 0
    except:
        return "N/A"

def get_agent_count():
    """获取Agent数量"""
    return len(st.session_state.get('agents', []))

def get_system_health():
    """获取系统健康度（备用函数）"""
    try:
        response = st.session_state.api_client.get_health()
        if response and 'data' in response:
            stats = response['data']
            if 'total_services' in stats and stats['total_services'] > 0:
                return int((stats.get('healthy_services', 0) / stats['total_services']) * 100)
        return 100
    except:
        return "N/A"

def show_service_status_chart():
    """显示服务状态图表"""
    try:
        # 使用缓存数据
        service_data = get_cached_service_data()
        services = service_data.get('services', [])

        if services:
            # 统计状态
            status_counts = {}
            for service in services:
                status = service.get('status', 'unknown')
                status_counts[status] = status_counts.get(status, 0) + 1

            if status_counts:
                st.bar_chart(status_counts)
            else:
                st.info("暂无服务数据")
        else:
            st.info("无法获取服务数据")
    except Exception as e:
        st.error(f"获取服务状态失败: {e}")

def show_recent_activities():
    """显示最近活动"""
    # 从操作历史获取真实活动
    history = SessionManager.get_operation_history(limit=5)

    if history:
        for item in history:
            timestamp = item['timestamp'].strftime('%H:%M:%S')
            operation = item['operation']
            st.text(f"[{timestamp}] {operation}")
    else:
        # 默认活动
        activities = [
            "🔄 服务 'mcpstore-wiki' 重启成功",
            "➕ 添加新服务 'demo-service'",
            "🧪 工具 'search_wiki' 测试完成",
            "👤 创建Agent 'knowledge-agent'"
        ]

        for activity in activities:
            st.text(activity)

def get_cached_service_data() -> Dict:
    """获取缓存的服务数据"""
    cached_data = SessionManager.get_cached_data('service_data', max_age_seconds=30)

    if cached_data:
        return cached_data

    # 获取新数据
    try:
        response = st.session_state.api_client.list_services()
        if response and 'data' in response:
            services = response['data']
            data = {
                'count': len(services),
                'healthy': sum(1 for s in services if s.get('status') == 'healthy'),
                'services': services
            }
        else:
            data = {'count': 0, 'healthy': 0, 'services': []}

        SessionManager.set_cached_data('service_data', data)
        return data
    except:
        return {'count': 0, 'healthy': 0, 'services': []}

def get_cached_tool_data() -> Dict:
    """获取缓存的工具数据"""
    cached_data = SessionManager.get_cached_data('tool_data', max_age_seconds=30)

    if cached_data:
        return cached_data

    # 获取新数据
    try:
        response = st.session_state.api_client.list_tools()
        if response and 'data' in response:
            tools = response['data']
            data = {
                'count': len(tools),
                'tools': tools
            }
        else:
            data = {'count': 0, 'tools': []}

        SessionManager.set_cached_data('tool_data', data)
        return data
    except:
        return {'count': 0, 'tools': []}

def calculate_system_health(service_data: Dict) -> int:
    """计算系统健康度"""
    total = service_data.get('count', 0)
    healthy = service_data.get('healthy', 0)

    if total == 0:
        return 100

    return int((healthy / total) * 100)

if __name__ == "__main__":
    main()
