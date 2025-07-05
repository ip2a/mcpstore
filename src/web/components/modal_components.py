"""
模态窗口组件
提供更好的弹窗体验
"""

import streamlit as st
from typing import Dict, List, Optional, Callable
import json

class ServiceModal:
    """服务相关的模态窗口"""
    
    @staticmethod
    def show_service_details(service: Dict):
        """显示服务详情模态窗口"""
        with st.container():
            st.markdown(f"### 🛠️ 服务详情: {service.get('name', 'Unknown')}")
            
            # 基本信息
            col1, col2 = st.columns(2)
            
            with col1:
                st.markdown("#### 📋 基本信息")
                st.write(f"**名称**: {service.get('name', 'N/A')}")
                st.write(f"**URL**: {service.get('url', 'N/A')}")
                st.write(f"**状态**: {service.get('status', 'N/A')}")
                st.write(f"**传输类型**: {service.get('transport', 'N/A')}")
            
            with col2:
                st.markdown("#### 📊 统计信息")
                tool_count = service.get('tool_count', 0)
                st.metric("工具数量", tool_count)
                
                # 状态指示
                status = service.get('status', 'unknown')
                status_color = "🟢" if status == 'healthy' else "🔴" if status == 'unhealthy' else "🟡"
                st.write(f"**状态**: {status_color} {status}")
            
            # 操作按钮
            st.markdown("#### ⚡ 快速操作")
            col1, col2, col3, col4 = st.columns(4)
            
            with col1:
                if st.button("🔄 重启服务", use_container_width=True):
                    ServiceModal._restart_service(service.get('name'))
            
            with col2:
                if st.button("🧪 测试连接", use_container_width=True):
                    ServiceModal._test_service(service.get('name'))
            
            with col3:
                if st.button("📊 查看工具", use_container_width=True):
                    st.session_state.show_service_tools = service.get('name')
            
            with col4:
                if st.button("🗑️ 删除服务", use_container_width=True):
                    st.session_state.confirm_delete_service = service.get('name')
    
    @staticmethod
    def _restart_service(service_name: str):
        """重启服务"""
        try:
            api_client = st.session_state.api_client
            response = api_client.restart_service(service_name)
            
            if response and response.get('success'):
                st.success(f"服务 {service_name} 重启成功")
            else:
                st.error(f"服务 {service_name} 重启失败")
        except Exception as e:
            st.error(f"重启服务时出错: {e}")
    
    @staticmethod
    def _test_service(service_name: str):
        """测试服务连接"""
        try:
            api_client = st.session_state.api_client
            response = api_client.get_service_status(service_name)
            
            if response and response.get('success'):
                st.success(f"服务 {service_name} 连接正常")
            else:
                st.error(f"服务 {service_name} 连接异常")
        except Exception as e:
            st.error(f"测试连接时出错: {e}")

class ToolModal:
    """工具相关的模态窗口"""
    
    @staticmethod
    def show_tool_tester(tool: Dict):
        """显示工具测试模态窗口"""
        st.markdown(f"### 🧪 测试工具: {tool.get('name', 'Unknown')}")
        
        # 工具信息
        col1, col2 = st.columns(2)
        
        with col1:
            st.write(f"**工具名称**: {tool.get('name', 'N/A')}")
            st.write(f"**所属服务**: {tool.get('service_name', 'N/A')}")
        
        with col2:
            st.write(f"**描述**: {tool.get('description', '无描述')}")
        
        # 参数表单
        schema = tool.get('inputSchema', {})
        if schema and 'properties' in schema:
            st.markdown("#### 📝 参数设置")
            
            form_data = {}
            properties = schema['properties']
            required_fields = schema.get('required', [])
            
            for param_name, param_info in properties.items():
                param_type = param_info.get('type', 'string')
                param_desc = param_info.get('description', '')
                is_required = param_name in required_fields
                
                label = f"{param_name}"
                if is_required:
                    label += " *"
                
                if param_type == 'string':
                    form_data[param_name] = st.text_input(
                        label, 
                        help=param_desc,
                        key=f"modal_tool_{tool.get('name')}_{param_name}"
                    )
                elif param_type in ['integer', 'number']:
                    form_data[param_name] = st.number_input(
                        label,
                        help=param_desc,
                        key=f"modal_tool_{tool.get('name')}_{param_name}"
                    )
                elif param_type == 'boolean':
                    form_data[param_name] = st.checkbox(
                        label,
                        help=param_desc,
                        key=f"modal_tool_{tool.get('name')}_{param_name}"
                    )
            
            # 执行按钮
            col1, col2 = st.columns(2)
            
            with col1:
                if st.button("🚀 执行工具", type="primary", use_container_width=True):
                    # 验证必需参数
                    missing_params = []
                    for param in required_fields:
                        if not form_data.get(param):
                            missing_params.append(param)
                    
                    if missing_params:
                        st.error(f"缺少必需参数: {', '.join(missing_params)}")
                    else:
                        # 清理空值
                        cleaned_data = {k: v for k, v in form_data.items() if v is not None and v != ''}
                        ToolModal._execute_tool(tool.get('name'), cleaned_data)
            
            with col2:
                if st.button("❌ 取消", use_container_width=True):
                    st.session_state.show_test_tool_modal = False
                    st.rerun()
        else:
            st.info("此工具无需参数")
            
            col1, col2 = st.columns(2)
            
            with col1:
                if st.button("🚀 执行工具", type="primary", use_container_width=True):
                    ToolModal._execute_tool(tool.get('name'), {})
            
            with col2:
                if st.button("❌ 取消", use_container_width=True):
                    st.session_state.show_test_tool_modal = False
                    st.rerun()
    
    @staticmethod
    def _execute_tool(tool_name: str, args: Dict):
        """执行工具"""
        try:
            api_client = st.session_state.api_client
            
            with st.spinner("执行工具中..."):
                response = api_client.use_tool(tool_name, args)
                
                if response and response.get('success'):
                    st.success("✅ 工具执行成功！")
                    
                    # 显示结果
                    if 'data' in response:
                        st.markdown("#### 📊 执行结果")
                        
                        result_data = response['data']
                        if isinstance(result_data, (dict, list)):
                            st.json(result_data)
                        else:
                            st.text(str(result_data))
                    
                    # 保存到历史
                    from utils.config_manager import SessionManager
                    SessionManager.add_operation_history(f"执行工具: {tool_name}")
                else:
                    st.error("❌ 工具执行失败")
        except Exception as e:
            st.error(f"执行工具时出错: {e}")

class ConfirmModal:
    """确认对话框模态窗口"""
    
    @staticmethod
    def show_delete_confirm(item_type: str, item_name: str, callback: Callable):
        """显示删除确认对话框"""
        st.markdown(f"### ⚠️ 确认删除")
        st.warning(f"您确定要删除{item_type} **{item_name}** 吗？")
        st.markdown("此操作无法撤销！")
        
        col1, col2 = st.columns(2)
        
        with col1:
            if st.button("🗑️ 确认删除", type="primary", use_container_width=True):
                callback(item_name)
                st.rerun()
        
        with col2:
            if st.button("❌ 取消", use_container_width=True):
                # 清除确认状态
                if f'confirm_delete_{item_type.lower()}' in st.session_state:
                    del st.session_state[f'confirm_delete_{item_type.lower()}']
                st.rerun()

class InfoModal:
    """信息展示模态窗口"""
    
    @staticmethod
    def show_system_info():
        """显示系统信息"""
        st.markdown("### 📊 系统详细信息")
        
        # 获取系统数据
        try:
            # API客户端信息
            api_client = st.session_state.api_client
            backend_info = api_client.get_backend_info()
            
            st.markdown("#### 🔧 API客户端")
            st.json(backend_info)
            
            # 缓存信息
            cache_info = {
                "缓存项数量": len(st.session_state.get('data_cache', {})),
                "操作历史": len(st.session_state.get('operation_history', [])),
                "通知数量": len(st.session_state.get('notifications', []))
            }
            
            st.markdown("#### 💾 缓存状态")
            st.json(cache_info)
            
            # 配置信息
            config_manager = st.session_state.config_manager
            config_info = {
                "后端类型": st.session_state.get('api_backend_type', 'unknown'),
                "API地址": st.session_state.get('api_base_url', 'unknown'),
                "预设服务数": len(config_manager.get_preset_services())
            }
            
            st.markdown("#### ⚙️ 配置信息")
            st.json(config_info)
            
        except Exception as e:
            st.error(f"获取系统信息失败: {e}")
        
        # 关闭按钮
        if st.button("❌ 关闭", use_container_width=True):
            st.session_state.show_system_info_modal = False
            st.rerun()
