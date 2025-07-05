"""
配置管理页面
"""

import streamlit as st
from typing import Dict
import json

from utils.helpers import (
    show_success_message, show_error_message, show_info_message,
    format_json, export_config, import_config
)

def show():
    """显示配置管理页面"""
    st.header("⚙️ 配置管理")
    
    # 创建标签页
    tab1, tab2, tab3 = st.tabs(["📋 查看配置", "✏️ 编辑配置", "🔄 配置操作"])
    
    with tab1:
        show_view_config()
    
    with tab2:
        show_edit_config()
    
    with tab3:
        show_config_operations()

def show_view_config():
    """显示查看配置页面"""
    st.subheader("📋 当前配置")
    
    # 操作按钮
    col1, col2, col3 = st.columns([1, 1, 2])
    
    with col1:
        if st.button("🔄 刷新配置", key="config_refresh"):
            st.rerun()
    
    with col2:
        config_type = st.selectbox(
            "配置类型",
            ["MCP配置", "系统配置"]
        )
    
    # 获取配置
    api_client = st.session_state.api_client
    
    if config_type == "MCP配置":
        # 对应API: GET /for_store/show_mcpconfig
        # 实际调用: store.for_store().show_mcpconfig()
        response = api_client.show_mcpconfig()
        config_title = "MCP服务配置"
    else:
        # 对应API: GET /for_store/get_config
        # 实际调用: store.for_store().get_config()
        response = api_client.get_config()
        config_title = "系统配置"
    
    if not response:
        show_error_message(f"无法获取{config_type}")
        return
    
    config_data = response.get('data', {})
    
    # 配置概览
    st.markdown(f"#### 📊 {config_title}概览")
    
    if config_type == "MCP配置" and 'mcpServers' in config_data:
        servers = config_data['mcpServers']
        
        col1, col2, col3 = st.columns(3)
        
        with col1:
            st.metric("服务数量", len(servers))
        
        with col2:
            # 统计传输类型
            transport_types = {}
            for server_config in servers.values():
                transport = server_config.get('transport', 'auto')
                transport_types[transport] = transport_types.get(transport, 0) + 1
            
            most_common = max(transport_types.items(), key=lambda x: x[1])[0] if transport_types else "无"
            st.metric("主要传输类型", most_common)
        
        with col3:
            # 统计有URL的服务
            url_count = sum(1 for config in servers.values() if 'url' in config)
            st.metric("URL服务", url_count)
        
        # 服务列表
        st.markdown("#### 🛠️ 已配置服务")
        
        for server_name, server_config in servers.items():
            with st.expander(f"🔧 {server_name}"):
                col1, col2 = st.columns(2)
                
                with col1:
                    st.write(f"**URL**: {server_config.get('url', 'N/A')}")
                    st.write(f"**传输类型**: {server_config.get('transport', 'auto')}")
                
                with col2:
                    if 'command' in server_config:
                        st.write(f"**命令**: {server_config['command']}")
                    
                    if 'args' in server_config:
                        st.write(f"**参数**: {server_config['args']}")
    
    # 完整配置展示
    st.markdown(f"#### 📄 完整{config_title}")
    
    # 格式选择
    format_option = st.radio(
        "显示格式",
        ["格式化JSON", "原始JSON", "表格视图"],
        horizontal=True
    )
    
    if format_option == "格式化JSON":
        st.json(config_data)
    elif format_option == "原始JSON":
        st.code(format_json(config_data), language='json')
    else:
        # 表格视图（仅适用于MCP配置）
        if config_type == "MCP配置" and 'mcpServers' in config_data:
            show_config_table(config_data['mcpServers'])
        else:
            st.info("表格视图仅适用于MCP配置")
    
    # 导出配置
    st.markdown("#### 📤 导出配置")
    
    if st.button("📥 下载配置文件"):
        config_str = export_config(config_data)
        from datetime import datetime
        st.download_button(
            label="💾 下载JSON文件",
            data=config_str,
            file_name=f"{config_type.lower().replace(' ', '_')}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json",
            mime="application/json"
        )

def show_edit_config():
    """显示编辑配置页面"""
    st.subheader("✏️ 编辑配置")
    
    st.warning("⚠️ 配置编辑功能正在开发中，请谨慎操作")
    
    # 配置编辑器
    st.markdown("#### 📝 配置编辑器")
    
    # 获取当前配置
    api_client = st.session_state.api_client
    # 对应API: GET /for_store/show_mcpconfig
    # 实际调用: store.for_store().show_mcpconfig()
    response = api_client.show_mcpconfig()
    
    if not response:
        show_error_message("无法获取当前配置")
        return
    
    current_config = response.get('data', {})
    
    # JSON编辑器
    config_text = st.text_area(
        "配置内容 (JSON格式)",
        value=format_json(current_config),
        height=400,
        help="直接编辑JSON配置，请确保格式正确"
    )
    
    # 验证和预览
    col1, col2 = st.columns(2)
    
    with col1:
        if st.button("🔍 验证配置"):
            validate_config(config_text)
    
    with col2:
        if st.button("👁️ 预览更改"):
            preview_config_changes(current_config, config_text)
    
    # 应用配置
    st.markdown("#### 💾 应用配置")
    
    col1, col2 = st.columns(2)
    
    with col1:
        if st.button("💾 保存配置", type="primary"):
            save_config(config_text)
    
    with col2:
        if st.button("🔄 重置为当前配置", key="config_reset_current"):
            st.rerun()

def show_config_operations():
    """显示配置操作页面"""
    st.subheader("🔄 配置操作")
    
    # 重置操作
    st.markdown("#### 🔄 重置配置")
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("**Store级别重置**")
        
        if st.button("🔄 重置Store配置", type="secondary", key="config_reset_store"):
            reset_store_config()
        
        st.caption("重置全局Store配置到默认状态")
    
    with col2:
        st.markdown("**Agent级别重置**")
        
        # Agent选择
        agents = st.session_state.get('agents', [])
        
        if agents:
            selected_agent = st.selectbox("选择Agent", agents)
            
            if st.button("🔄 重置Agent配置", type="secondary", key="config_reset_agent"):
                reset_agent_config(selected_agent)
            
            st.caption(f"重置Agent {selected_agent} 的配置")
        else:
            st.info("暂无可重置的Agent")
    
    # 导入导出操作
    st.markdown("#### 📁 导入导出")
    
    col1, col2 = st.columns(2)
    
    with col1:
        st.markdown("**导入配置**")
        
        uploaded_file = st.file_uploader(
            "选择配置文件",
            type=['json'],
            help="上传JSON格式的配置文件"
        )
        
        if uploaded_file and st.button("📤 导入配置"):
            import_config_file(uploaded_file)
    
    with col2:
        st.markdown("**导出配置**")
        
        export_type = st.selectbox(
            "导出类型",
            ["MCP配置", "完整配置"]
        )
        
        if st.button("📥 导出配置"):
            export_current_config(export_type)
    
    # 备份恢复
    st.markdown("#### 💾 备份恢复")
    
    col1, col2 = st.columns(2)
    
    with col1:
        if st.button("💾 创建备份"):
            create_config_backup()
    
    with col2:
        if st.button("🔙 恢复默认配置"):
            restore_default_config()

def show_config_table(servers_config: Dict):
    """以表格形式显示配置"""
    import pandas as pd
    
    # 转换为表格数据
    table_data = []
    
    for server_name, server_config in servers_config.items():
        row = {
            "服务名": server_name,
            "URL": server_config.get('url', ''),
            "传输类型": server_config.get('transport', 'auto'),
            "命令": server_config.get('command', ''),
            "参数": str(server_config.get('args', [])) if 'args' in server_config else ''
        }
        table_data.append(row)
    
    if table_data:
        df = pd.DataFrame(table_data)
        st.dataframe(df, use_container_width=True)
    else:
        st.info("无配置数据")

def validate_config(config_text: str):
    """验证配置"""
    try:
        config = json.loads(config_text)
        
        # 基本格式验证
        if not isinstance(config, dict):
            show_error_message("配置必须是JSON对象格式")
            return
        
        # MCP配置验证
        if 'mcpServers' in config:
            servers = config['mcpServers']
            
            if not isinstance(servers, dict):
                show_error_message("mcpServers必须是对象格式")
                return
            
            # 验证每个服务配置
            for server_name, server_config in servers.items():
                if not isinstance(server_config, dict):
                    show_error_message(f"服务 {server_name} 配置格式错误")
                    return
                
                # 检查必需字段
                if 'url' not in server_config and 'command' not in server_config:
                    show_error_message(f"服务 {server_name} 缺少url或command字段")
                    return
        
        show_success_message("✅ 配置格式验证通过")
        
    except json.JSONDecodeError as e:
        show_error_message(f"JSON格式错误: {e}")

def preview_config_changes(current_config: Dict, new_config_text: str):
    """预览配置更改"""
    try:
        new_config = json.loads(new_config_text)
        
        st.markdown("#### 🔍 配置更改预览")
        
        # 简单的差异比较
        if current_config == new_config:
            st.info("配置无更改")
            return
        
        # 显示主要差异
        col1, col2 = st.columns(2)
        
        with col1:
            st.markdown("**当前配置**")
            st.code(format_json(current_config)[:500] + "...", language='json')
        
        with col2:
            st.markdown("**新配置**")
            st.code(format_json(new_config)[:500] + "...", language='json')
        
        show_info_message("配置已更改，请仔细检查后保存")
        
    except json.JSONDecodeError:
        show_error_message("新配置JSON格式错误，无法预览")

def save_config(config_text: str):
    """保存配置"""
    try:
        config = json.loads(config_text)
        
        # 这里应该调用相应的API保存配置
        # 由于当前API可能不支持直接保存配置，这里只是示例
        
        show_info_message("配置保存功能正在开发中")
        
    except json.JSONDecodeError:
        show_error_message("配置格式错误，无法保存")

def reset_store_config():
    """重置Store配置"""
    api_client = st.session_state.api_client
    
    with st.spinner("重置Store配置..."):
        # 对应API: POST /for_store/reset_config
        # 实际调用: store.for_store().reset_config()
        response = api_client.reset_config()
        
        if response and response.get('success'):
            show_success_message("Store配置重置成功")
            st.rerun()
        else:
            show_error_message("Store配置重置失败")

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

def import_config_file(uploaded_file):
    """导入配置文件"""
    try:
        content = uploaded_file.read().decode('utf-8')
        config = import_config(content)
        
        if config:
            st.session_state.imported_config = config
            show_success_message("配置文件导入成功，请在编辑页面中应用")
        
    except Exception as e:
        show_error_message(f"导入配置文件失败: {e}")

def export_current_config(export_type: str):
    """导出当前配置"""
    api_client = st.session_state.api_client
    
    if export_type == "MCP配置":
        response = api_client.show_mcpconfig()
    else:
        response = api_client.get_config()
    
    if response:
        config_data = response.get('data', {})
        config_str = export_config(config_data)
        
        from datetime import datetime
        filename = f"{export_type.lower().replace(' ', '_')}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        
        st.download_button(
            label="💾 下载配置文件",
            data=config_str,
            file_name=filename,
            mime="application/json"
        )
    else:
        show_error_message("无法获取配置数据")

def create_config_backup():
    """创建配置备份"""
    show_info_message("配置备份功能正在开发中")

def restore_default_config():
    """恢复默认配置"""
    show_info_message("恢复默认配置功能正在开发中")
