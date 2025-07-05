"""
服务管理专用组件
提供丝滑的服务管理体验
"""

import streamlit as st
from typing import Dict, List, Optional, Callable
from datetime import datetime
import json

from .ui_components import StatusIndicator, DataTable, ConfirmDialog, MetricCard
from utils.config_manager import SessionManager

class ServiceCard:
    """增强的服务卡片组件"""
    
    @staticmethod
    def show(service: Dict, actions: List[Dict] = None):
        """显示服务卡片"""
        with st.container():
            # 卡片样式
            card_style = """
            <div style="
                border: 1px solid #ddd;
                border-radius: 8px;
                padding: 16px;
                margin: 8px 0;
                background: white;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            ">
            """
            
            col1, col2, col3, col4 = st.columns([3, 1, 1, 2])
            
            with col1:
                # 服务基本信息
                status = service.get('status', 'unknown')
                status_display = StatusIndicator.show(status, size="small")
                
                st.markdown(f"**{service.get('name', 'Unknown')}** {status_display}")
                st.caption(service.get('url', 'No URL'))
                
                # 服务标签
                ServiceCard._show_service_tags(service)
            
            with col2:
                # 工具数量
                tool_count = service.get('tool_count', 0)
                st.metric("工具", tool_count, help="可用工具数量")
            
            with col3:
                # 连接时间
                ServiceCard._show_connection_time(service)
            
            with col4:
                # 操作按钮
                ServiceCard._show_action_buttons(service, actions)
    
    @staticmethod
    def _show_service_tags(service: Dict):
        """显示服务标签"""
        tags = []
        
        # 传输类型标签
        transport = service.get('transport', 'auto')
        if transport != 'auto':
            tags.append(f"🔗 {transport}")
        
        # 健康状态标签
        status = service.get('status', 'unknown')
        if status == 'healthy':
            tags.append("✅ 健康")
        elif status == 'unhealthy':
            tags.append("❌ 异常")
        
        # 显示标签
        if tags:
            st.caption(" | ".join(tags))
    
    @staticmethod
    def _show_connection_time(service: Dict):
        """显示连接时间信息"""
        # 这里可以显示连接时间、响应时间等
        st.metric("响应", "< 100ms", help="平均响应时间")
    
    @staticmethod
    def _show_action_buttons(service: Dict, actions: List[Dict]):
        """显示操作按钮"""
        if not actions:
            actions = [
                {'key': 'restart', 'icon': '🔄', 'label': '重启', 'help': '重启服务'},
                {'key': 'edit', 'icon': '✏️', 'label': '编辑', 'help': '编辑配置'},
                {'key': 'delete', 'icon': '🗑️', 'label': '删除', 'help': '删除服务'}
            ]
        
        # 创建按钮行
        button_cols = st.columns(len(actions))
        
        for i, action in enumerate(actions):
            with button_cols[i]:
                button_key = f"{action['key']}_{service.get('name', '')}"
                
                if st.button(
                    action['icon'],
                    key=button_key,
                    help=action.get('help', action.get('label', '')),
                    use_container_width=True
                ):
                    # 触发操作
                    ServiceCard._handle_action(service, action)
    
    @staticmethod
    def _handle_action(service: Dict, action: Dict):
        """处理操作"""
        service_name = service.get('name', '')
        action_key = action['key']
        
        # 记录操作历史
        SessionManager.add_operation_history(
            f"{action.get('label', action_key)} 服务: {service_name}",
            {'service': service_name, 'action': action_key}
        )
        
        # 设置会话状态
        st.session_state[f'service_action_{action_key}'] = service

class ServiceWizard:
    """服务添加向导"""
    
    @staticmethod
    def show():
        """显示服务添加向导"""
        st.subheader("🧙‍♂️ 服务添加向导")
        
        # 步骤指示器
        ServiceWizard._show_step_indicator()
        
        # 获取当前步骤
        current_step = st.session_state.get('wizard_step', 1)
        
        if current_step == 1:
            ServiceWizard._step_1_service_type()
        elif current_step == 2:
            ServiceWizard._step_2_basic_config()
        elif current_step == 3:
            ServiceWizard._step_3_advanced_config()
        elif current_step == 4:
            ServiceWizard._step_4_confirmation()
    
    @staticmethod
    def _show_step_indicator():
        """显示步骤指示器"""
        current_step = st.session_state.get('wizard_step', 1)
        
        steps = [
            "1️⃣ 选择类型",
            "2️⃣ 基本配置", 
            "3️⃣ 高级配置",
            "4️⃣ 确认添加"
        ]
        
        # 创建步骤指示器
        cols = st.columns(len(steps))
        
        for i, step in enumerate(steps, 1):
            with cols[i-1]:
                if i == current_step:
                    st.markdown(f"**{step}** ⬅️")
                elif i < current_step:
                    st.markdown(f"~~{step}~~ ✅")
                else:
                    st.markdown(f"{step}")
        
        st.markdown("---")
    
    @staticmethod
    def _step_1_service_type():
        """步骤1: 选择服务类型"""
        st.markdown("#### 选择服务类型")
        
        # 预设服务
        config_manager = st.session_state.config_manager
        preset_services = config_manager.get_preset_services()
        
        service_type = st.radio(
            "服务类型",
            ["预设服务", "自定义服务"],
            horizontal=True
        )
        
        if service_type == "预设服务":
            if preset_services:
                selected_preset = st.selectbox(
                    "选择预设服务",
                    preset_services,
                    format_func=lambda x: f"{x['name']} - {x['description']}"
                )
                
                if selected_preset:
                    st.session_state.wizard_config = selected_preset.copy()
                    st.json(selected_preset)
            else:
                st.info("暂无预设服务")
        else:
            st.session_state.wizard_config = {
                'name': '',
                'url': '',
                'transport': 'auto'
            }
            st.info("将配置自定义服务")
        
        # 下一步按钮
        if st.button("下一步 ➡️", type="primary"):
            st.session_state.wizard_step = 2
            st.rerun()
    
    @staticmethod
    def _step_2_basic_config():
        """步骤2: 基本配置"""
        st.markdown("#### 基本配置")
        
        config = st.session_state.get('wizard_config', {})
        
        # 基本信息表单
        with st.form("basic_config_form"):
            name = st.text_input("服务名称", value=config.get('name', ''))
            url = st.text_input("服务URL", value=config.get('url', ''))
            transport = st.selectbox(
                "传输类型",
                ["auto", "sse", "streamable-http"],
                index=["auto", "sse", "streamable-http"].index(config.get('transport', 'auto'))
            )
            
            description = st.text_area("描述", value=config.get('description', ''))
            
            col1, col2 = st.columns(2)
            
            with col1:
                if st.form_submit_button("⬅️ 上一步"):
                    st.session_state.wizard_step = 1
                    st.rerun()
            
            with col2:
                if st.form_submit_button("下一步 ➡️", type="primary"):
                    # 保存配置
                    st.session_state.wizard_config.update({
                        'name': name,
                        'url': url,
                        'transport': transport,
                        'description': description
                    })
                    st.session_state.wizard_step = 3
                    st.rerun()
    
    @staticmethod
    def _step_3_advanced_config():
        """步骤3: 高级配置"""
        st.markdown("#### 高级配置")
        
        config = st.session_state.get('wizard_config', {})
        
        with st.form("advanced_config_form"):
            # 高级选项
            keep_alive = st.checkbox("保持连接", value=config.get('keep_alive', False))
            
            headers_text = st.text_area(
                "自定义请求头 (JSON)",
                value=json.dumps(config.get('headers', {}), indent=2) if config.get('headers') else '',
                help="JSON格式的HTTP请求头"
            )
            
            env_text = st.text_area(
                "环境变量 (JSON)",
                value=json.dumps(config.get('env', {}), indent=2) if config.get('env') else '',
                help="JSON格式的环境变量"
            )
            
            col1, col2 = st.columns(2)
            
            with col1:
                if st.form_submit_button("⬅️ 上一步"):
                    st.session_state.wizard_step = 2
                    st.rerun()
            
            with col2:
                if st.form_submit_button("下一步 ➡️", type="primary"):
                    # 保存高级配置
                    advanced_config = {'keep_alive': keep_alive}
                    
                    if headers_text.strip():
                        try:
                            advanced_config['headers'] = json.loads(headers_text)
                        except:
                            st.error("请求头JSON格式错误")
                            return
                    
                    if env_text.strip():
                        try:
                            advanced_config['env'] = json.loads(env_text)
                        except:
                            st.error("环境变量JSON格式错误")
                            return
                    
                    st.session_state.wizard_config.update(advanced_config)
                    st.session_state.wizard_step = 4
                    st.rerun()
    
    @staticmethod
    def _step_4_confirmation():
        """步骤4: 确认添加"""
        st.markdown("#### 确认配置")
        
        config = st.session_state.get('wizard_config', {})
        
        # 显示最终配置
        st.json(config)
        
        col1, col2, col3 = st.columns(3)
        
        with col1:
            if st.button("⬅️ 上一步"):
                st.session_state.wizard_step = 3
                st.rerun()
        
        with col2:
            if st.button("🔄 重新开始"):
                ServiceWizard._reset_wizard()
                st.rerun()
        
        with col3:
            if st.button("✅ 确认添加", type="primary"):
                ServiceWizard._add_service(config)

    @staticmethod
    def _add_service(config: Dict):
        """添加服务"""
        try:
            api_client = st.session_state.api_client
            response = api_client.add_service(config)
            
            if response and response.get('success'):
                SessionManager.add_notification(f"服务 {config['name']} 添加成功！", "success")
                SessionManager.add_operation_history(f"添加服务: {config['name']}")
                ServiceWizard._reset_wizard()
                st.rerun()
            else:
                SessionManager.add_notification(f"服务 {config['name']} 添加失败", "error")
        except Exception as e:
            SessionManager.add_notification(f"添加服务时出错: {e}", "error")
    
    @staticmethod
    def _reset_wizard():
        """重置向导"""
        if 'wizard_step' in st.session_state:
            del st.session_state.wizard_step
        if 'wizard_config' in st.session_state:
            del st.session_state.wizard_config

class ServiceMonitor:
    """服务监控组件"""
    
    @staticmethod
    def show_realtime_status():
        """显示实时状态"""
        st.subheader("📊 实时服务状态")
        
        # 获取服务数据
        from utils.config_manager import SessionManager
        
        # 使用较短的缓存时间以获得更实时的数据
        cached_data = SessionManager.get_cached_data('realtime_service_data', max_age_seconds=5)
        
        if not cached_data:
            # 获取实时数据
            try:
                api_client = st.session_state.api_client
                response = api_client.list_services()
                
                if response and 'data' in response:
                    services = response['data']
                    
                    # 计算统计信息
                    total = len(services)
                    healthy = sum(1 for s in services if s.get('status') == 'healthy')
                    unhealthy = sum(1 for s in services if s.get('status') == 'unhealthy')
                    unknown = total - healthy - unhealthy
                    
                    cached_data = {
                        'total': total,
                        'healthy': healthy,
                        'unhealthy': unhealthy,
                        'unknown': unknown,
                        'services': services,
                        'timestamp': datetime.now()
                    }
                    
                    SessionManager.set_cached_data('realtime_service_data', cached_data)
                else:
                    cached_data = {'total': 0, 'healthy': 0, 'unhealthy': 0, 'unknown': 0, 'services': []}
            except:
                cached_data = {'total': 0, 'healthy': 0, 'unhealthy': 0, 'unknown': 0, 'services': []}
        
        # 显示统计卡片
        col1, col2, col3, col4 = st.columns(4)
        
        with col1:
            MetricCard.show("总服务", cached_data['total'], icon="🛠️")
        
        with col2:
            MetricCard.show("健康", cached_data['healthy'], icon="✅", color="green")
        
        with col3:
            MetricCard.show("异常", cached_data['unhealthy'], icon="❌", color="red")
        
        with col4:
            MetricCard.show("未知", cached_data['unknown'], icon="❓", color="gray")
        
        # 显示更新时间
        if 'timestamp' in cached_data:
            st.caption(f"更新时间: {cached_data['timestamp'].strftime('%H:%M:%S')}")
        
        # 自动刷新选项
        auto_refresh = st.checkbox("自动刷新 (5秒)", value=False)
        
        if auto_refresh:
            import time
            time.sleep(5)
            st.rerun()
