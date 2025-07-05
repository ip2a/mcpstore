"""
Agent管理页面
"""

import streamlit as st
from typing import Dict, List

from utils.helpers import (
    show_success_message, show_error_message, show_info_message, show_warning_message,
    create_agent_card, format_json
)

def show():
    """显示Agent管理页面"""
    st.header("👥 Agent管理")
    
    # 创建标签页
    tab1, tab2, tab3 = st.tabs(["📋 Agent列表", "➕ 创建Agent", "🔧 Agent配置"])
    
    with tab1:
        show_agent_list()
    
    with tab2:
        show_create_agent()
    
    with tab3:
        show_agent_config()

def show_agent_list():
    """显示Agent列表"""
    st.subheader("📋 已创建的Agent")
    
    # 操作按钮
    col1, col2 = st.columns([1, 3])

    with col1:
        if st.button("🔄 刷新列表", key="agent_refresh_list"):
            st.rerun()
    
    # 获取Agent列表
    agents = st.session_state.get('agents', [])
    
    if not agents:
        st.info("暂无Agent，请创建一个新的Agent")
        return
    
    # Agent统计
    st.metric("Agent总数", len(agents))
    
    # Agent列表
    for agent_id in agents:
        with st.container():
            col1, col2, col3, col4 = st.columns([2, 1, 1, 2])
            
            with col1:
                st.markdown(f"**👤 {agent_id}**")
                
                # 获取Agent服务数量
                service_count = get_agent_service_count(agent_id)
                st.caption(f"服务数: {service_count}")
            
            with col2:
                # 获取工具数量
                tool_count = get_agent_tool_count(agent_id)
                st.metric("工具", tool_count)
            
            with col3:
                # Agent状态
                status = get_agent_status(agent_id)
                status_icon = "🟢" if status == "active" else "🟡"
                st.write(f"{status_icon} {status}")
            
            with col4:
                # 操作按钮
                col4_1, col4_2, col4_3 = st.columns(3)
                
                with col4_1:
                    if st.button("🔧", key=f"config_{agent_id}", help="配置Agent"):
                        st.session_state.selected_agent = agent_id
                        st.rerun()
                
                with col4_2:
                    if st.button("📊", key=f"stats_{agent_id}", help="查看统计"):
                        show_agent_stats(agent_id)
                
                with col4_3:
                    if st.button("🗑️", key=f"delete_{agent_id}", help="删除Agent"):
                        delete_agent(agent_id)
            
            st.markdown("---")

def show_create_agent():
    """显示创建Agent页面"""
    st.subheader("➕ 创建新Agent")
    
    with st.form("create_agent_form"):
        col1, col2 = st.columns(2)
        
        with col1:
            agent_id = st.text_input(
                "Agent ID *",
                help="Agent的唯一标识符"
            )
            
            agent_description = st.text_area(
                "描述",
                help="Agent的功能描述"
            )
        
        with col2:
            # 预设Agent类型
            agent_type = st.selectbox(
                "Agent类型",
                ["通用助手", "知识管理", "开发支持", "数据分析", "自定义"]
            )
            
            # 初始服务配置
            init_services = st.multiselect(
                "初始服务",
                get_available_services(),
                help="为Agent分配初始服务"
            )
        
        submitted = st.form_submit_button("🚀 创建Agent")
        
        if submitted:
            create_agent(agent_id, agent_description, agent_type, init_services)

def show_agent_config():
    """显示Agent配置页面"""
    selected_agent = st.session_state.get('selected_agent')
    
    if not selected_agent:
        st.info("请从Agent列表中选择一个Agent进行配置")
        return
    
    st.subheader(f"🔧 配置Agent: {selected_agent}")
    
    # Agent基本信息
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("#### 📋 基本信息")
        st.write(f"**Agent ID**: {selected_agent}")
        
        # 获取Agent服务列表
        services = get_agent_services(selected_agent)
        st.write(f"**服务数量**: {len(services)}")
        
        # 获取工具数量
        tool_count = get_agent_tool_count(selected_agent)
        st.write(f"**工具数量**: {tool_count}")
    
    with col2:
        st.markdown("#### ⚙️ 操作")
        
        if st.button("🔄 重置配置"):
            reset_agent_config(selected_agent)
        
        if st.button("📊 查看统计"):
            show_agent_stats(selected_agent)
        
        if st.button("🧪 测试工具"):
            st.session_state.test_agent_tools = selected_agent
    
    # 服务管理
    st.markdown("#### 🛠️ 服务管理")
    
    # 当前服务
    services = get_agent_services(selected_agent)
    
    if services:
        st.markdown("**已分配服务**:")
        for service in services:
            col1, col2, col3 = st.columns([3, 1, 1])
            
            with col1:
                st.write(f"🛠️ {service.get('name', 'Unknown')}")
            
            with col2:
                tool_count = service.get('tool_count', 0)
                st.write(f"工具: {tool_count}")
            
            with col3:
                if st.button("移除", key=f"remove_{service.get('name')}"):
                    remove_agent_service(selected_agent, service.get('name'))
    else:
        st.info("暂无分配的服务")
    
    # 添加服务
    st.markdown("**添加新服务**:")
    
    available_services = get_available_services()
    current_service_names = [s.get('name') for s in services]
    
    # 过滤已分配的服务
    new_services = [s for s in available_services if s not in current_service_names]
    
    if new_services:
        selected_services = st.multiselect(
            "选择要添加的服务",
            new_services
        )
        
        if selected_services and st.button("➕ 添加服务"):
            add_agent_services(selected_agent, selected_services)
    else:
        st.info("所有可用服务都已分配")

# ==================== 辅助函数 ====================

def get_agent_service_count(agent_id: str) -> int:
    """获取Agent服务数量"""
    api_client = st.session_state.api_client
    response = api_client.list_agent_services(agent_id)
    
    if response and 'data' in response:
        return len(response['data'])
    return 0

def get_agent_tool_count(agent_id: str) -> int:
    """获取Agent工具数量"""
    api_client = st.session_state.api_client
    response = api_client.list_agent_tools(agent_id)
    
    if response and 'data' in response:
        return len(response['data'])
    return 0

def get_agent_status(agent_id: str) -> str:
    """获取Agent状态"""
    # 简单的状态判断
    service_count = get_agent_service_count(agent_id)
    return "active" if service_count > 0 else "inactive"

def get_agent_services(agent_id: str) -> List[Dict]:
    """获取Agent服务列表"""
    api_client = st.session_state.api_client
    response = api_client.list_agent_services(agent_id)
    
    if response and 'data' in response:
        return response['data']
    return []

def get_available_services() -> List[str]:
    """获取可用服务列表"""
    api_client = st.session_state.api_client
    response = api_client.list_services()
    
    if response and 'data' in response:
        return [service.get('name') for service in response['data']]
    return []

def create_agent(agent_id: str, description: str, agent_type: str, init_services: List[str]):
    """创建Agent"""
    if not agent_id.strip():
        show_error_message("Agent ID不能为空")
        return
    
    # 检查Agent是否已存在
    agents = st.session_state.get('agents', [])
    if agent_id in agents:
        show_error_message(f"Agent {agent_id} 已存在")
        return
    
    # 添加到Agent列表
    agents.append(agent_id)
    st.session_state.agents = agents
    
    # 如果有初始服务，添加到Agent
    if init_services:
        add_agent_services(agent_id, init_services)
    
    show_success_message(f"Agent {agent_id} 创建成功")
    st.rerun()

def delete_agent(agent_id: str):
    """删除Agent"""
    # 确认删除
    if not st.session_state.get(f'confirm_delete_agent_{agent_id}'):
        st.session_state[f'confirm_delete_agent_{agent_id}'] = True
        show_warning_message(f"确认删除Agent {agent_id}？再次点击删除按钮确认。")
        return
    
    # 从列表中移除
    agents = st.session_state.get('agents', [])
    if agent_id in agents:
        agents.remove(agent_id)
        st.session_state.agents = agents
    
    # 清理确认状态
    if f'confirm_delete_agent_{agent_id}' in st.session_state:
        del st.session_state[f'confirm_delete_agent_{agent_id}']
    
    show_success_message(f"Agent {agent_id} 删除成功")
    st.rerun()

def add_agent_services(agent_id: str, service_names: List[str]):
    """为Agent添加服务"""
    api_client = st.session_state.api_client
    
    success_count = 0
    
    with st.spinner(f"为Agent {agent_id} 添加服务..."):
        for service_name in service_names:
            response = api_client.add_agent_service(agent_id, [service_name])
            if response and response.get('success'):
                success_count += 1
    
    show_success_message(f"成功为Agent {agent_id} 添加 {success_count}/{len(service_names)} 个服务")
    st.rerun()

def remove_agent_service(agent_id: str, service_name: str):
    """移除Agent服务"""
    api_client = st.session_state.api_client
    
    with st.spinner(f"移除服务 {service_name}..."):
        response = api_client.delete_agent_service(agent_id, service_name)
        
        if response and response.get('success'):
            show_success_message(f"成功移除服务 {service_name}")
            st.rerun()
        else:
            show_error_message(f"移除服务 {service_name} 失败")

def reset_agent_config(agent_id: str):
    """重置Agent配置"""
    api_client = st.session_state.api_client
    
    with st.spinner(f"重置Agent {agent_id} 配置..."):
        response = api_client.reset_agent_config(agent_id)
        
        if response and response.get('success'):
            show_success_message(f"Agent {agent_id} 配置重置成功")
            st.rerun()
        else:
            show_error_message(f"Agent {agent_id} 配置重置失败")

def show_agent_stats(agent_id: str):
    """显示Agent统计信息"""
    api_client = st.session_state.api_client
    response = api_client.get_agent_stats(agent_id)
    
    if response and 'data' in response:
        stats = response['data']
        
        with st.expander(f"📊 Agent {agent_id} 统计信息", expanded=True):
            col1, col2, col3 = st.columns(3)
            
            with col1:
                st.metric("服务数", stats.get('service_count', 0))
            
            with col2:
                st.metric("工具数", stats.get('tool_count', 0))
            
            with col3:
                st.metric("健康服务", stats.get('healthy_services', 0))
    else:
        show_error_message(f"无法获取Agent {agent_id} 的统计信息")
