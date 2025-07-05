"""
增强的UI组件库
提供丝滑的用户界面组件
"""

import streamlit as st
from typing import Dict, List, Optional, Any, Callable
from datetime import datetime
import time
import json

class StatusIndicator:
    """状态指示器组件"""
    
    @staticmethod
    def show(status: str, text: str = None, size: str = "normal") -> str:
        """显示状态指示器"""
        status_config = {
            'healthy': {'icon': '🟢', 'color': 'green', 'text': '健康'},
            'unhealthy': {'icon': '🔴', 'color': 'red', 'text': '异常'},
            'warning': {'icon': '🟡', 'color': 'orange', 'text': '警告'},
            'unknown': {'icon': '⚪', 'color': 'gray', 'text': '未知'},
            'connecting': {'icon': '🟠', 'color': 'orange', 'text': '连接中'},
            'disconnected': {'icon': '⚫', 'color': 'gray', 'text': '已断开'},
            'active': {'icon': '🟢', 'color': 'green', 'text': '活跃'},
            'inactive': {'icon': '🔴', 'color': 'red', 'text': '非活跃'}
        }
        
        config = status_config.get(status.lower(), status_config['unknown'])
        display_text = text or config['text']
        
        if size == "small":
            return f"{config['icon']} {display_text}"
        else:
            return f"**{config['icon']} {display_text}**"

class MetricCard:
    """指标卡片组件"""
    
    @staticmethod
    def show(title: str, value: Any, delta: Any = None, help_text: str = None,
             color: str = None, icon: str = None):
        """显示指标卡片"""
        with st.container():
            if icon:
                st.markdown(f"### {icon} {title}")
            else:
                st.markdown(f"### {title}")

            # 主要数值
            if color:
                st.markdown(f"<h2 style='color: {color}; margin: 0;'>{value}</h2>",
                           unsafe_allow_html=True)
            else:
                # 使用title作为label，并隐藏显示
                st.metric(title, value, delta, label_visibility="collapsed")

            # 帮助文本
            if help_text:
                st.caption(help_text)

class ProgressBar:
    """进度条组件"""
    
    @staticmethod
    def show(progress: float, text: str = None, color: str = "blue"):
        """显示进度条"""
        if text:
            st.text(text)
        
        # 创建进度条HTML
        progress_html = f"""
        <div style="
            width: 100%;
            background-color: #f0f0f0;
            border-radius: 10px;
            overflow: hidden;
            height: 20px;
        ">
            <div style="
                width: {progress}%;
                background-color: {color};
                height: 100%;
                transition: width 0.3s ease;
            "></div>
        </div>
        """
        
        st.markdown(progress_html, unsafe_allow_html=True)
        st.caption(f"{progress:.1f}%")

class NotificationSystem:
    """通知系统组件"""
    
    @staticmethod
    def show_notifications():
        """显示通知"""
        from utils.config_manager import SessionManager
        
        notifications = SessionManager.get_active_notifications()
        
        if not notifications:
            return
        
        # 创建通知容器
        notification_container = st.container()
        
        with notification_container:
            for notification in notifications:
                NotificationSystem._render_notification(notification)
    
    @staticmethod
    def _render_notification(notification: Dict):
        """渲染单个通知"""
        type_config = {
            'info': {'color': '#17a2b8', 'icon': 'ℹ️'},
            'success': {'color': '#28a745', 'icon': '✅'},
            'warning': {'color': '#ffc107', 'icon': '⚠️'},
            'error': {'color': '#dc3545', 'icon': '❌'}
        }
        
        config = type_config.get(notification['type'], type_config['info'])
        
        # 通知HTML
        notification_html = f"""
        <div style="
            background-color: {config['color']}20;
            border-left: 4px solid {config['color']};
            padding: 10px;
            margin: 5px 0;
            border-radius: 4px;
            position: relative;
        ">
            <div style="display: flex; align-items: center;">
                <span style="margin-right: 8px;">{config['icon']}</span>
                <span>{notification['message']}</span>
                <button onclick="this.parentElement.parentElement.style.display='none'" 
                        style="
                            margin-left: auto;
                            background: none;
                            border: none;
                            font-size: 16px;
                            cursor: pointer;
                        ">×</button>
            </div>
        </div>
        """
        
        st.markdown(notification_html, unsafe_allow_html=True)

class DataTable:
    """数据表格组件"""
    
    @staticmethod
    def show(data: List[Dict], columns: List[Dict], 
             actions: List[Dict] = None, 
             search: bool = True,
             pagination: bool = True,
             page_size: int = 10):
        """
        显示数据表格
        
        Args:
            data: 数据列表
            columns: 列配置 [{'key': 'name', 'title': '名称', 'type': 'text'}]
            actions: 操作按钮 [{'label': '编辑', 'key': 'edit', 'icon': '✏️'}]
            search: 是否显示搜索
            pagination: 是否分页
            page_size: 每页大小
        """
        
        # 搜索功能
        filtered_data = data
        if search and data:
            search_term = st.text_input("🔍 搜索", key="table_search")
            if search_term:
                filtered_data = DataTable._filter_data(data, search_term, columns)
        
        # 分页功能
        if pagination and len(filtered_data) > page_size:
            total_pages = (len(filtered_data) - 1) // page_size + 1
            
            col1, col2, col3 = st.columns([1, 2, 1])
            with col2:
                page = st.selectbox(
                    "页码", 
                    range(1, total_pages + 1),
                    format_func=lambda x: f"第 {x} 页 (共 {total_pages} 页)"
                )
            
            start_idx = (page - 1) * page_size
            end_idx = start_idx + page_size
            page_data = filtered_data[start_idx:end_idx]
        else:
            page_data = filtered_data
        
        # 表格渲染
        if not page_data:
            st.info("暂无数据")
            return
        
        # 表头
        header_cols = st.columns([col.get('width', 1) for col in columns] + ([1] if actions else []))
        
        for i, col_config in enumerate(columns):
            with header_cols[i]:
                st.markdown(f"**{col_config['title']}**")
        
        if actions:
            with header_cols[-1]:
                st.markdown("**操作**")
        
        # 数据行
        for row_idx, row in enumerate(page_data):
            cols = st.columns([col.get('width', 1) for col in columns] + ([1] if actions else []))
            
            for i, col_config in enumerate(columns):
                with cols[i]:
                    value = row.get(col_config['key'], '')
                    DataTable._render_cell(value, col_config)
            
            # 操作按钮
            if actions:
                with cols[-1]:
                    DataTable._render_actions(row, actions, row_idx)
    
    @staticmethod
    def _filter_data(data: List[Dict], search_term: str, columns: List[Dict]) -> List[Dict]:
        """过滤数据"""
        search_term = search_term.lower()
        filtered = []
        
        for row in data:
            for col in columns:
                value = str(row.get(col['key'], '')).lower()
                if search_term in value:
                    filtered.append(row)
                    break
        
        return filtered
    
    @staticmethod
    def _render_cell(value: Any, col_config: Dict):
        """渲染单元格"""
        cell_type = col_config.get('type', 'text')
        
        if cell_type == 'status':
            st.markdown(StatusIndicator.show(str(value)))
        elif cell_type == 'metric':
            st.metric("", value)
        elif cell_type == 'progress':
            ProgressBar.show(float(value) if isinstance(value, (int, float)) else 0)
        else:
            st.write(value)
    
    @staticmethod
    def _render_actions(row: Dict, actions: List[Dict], row_idx: int):
        """渲染操作按钮"""
        action_cols = st.columns(len(actions))
        
        for i, action in enumerate(actions):
            with action_cols[i]:
                button_key = f"{action['key']}_{row_idx}_{row.get('id', '')}"
                
                if st.button(
                    action.get('icon', '') + action['label'],
                    key=button_key,
                    help=action.get('help', '')
                ):
                    # 触发回调
                    if 'callback' in action:
                        action['callback'](row)
                    else:
                        # 设置会话状态
                        st.session_state[f"action_{action['key']}"] = row

class LoadingSpinner:
    """加载动画组件"""
    
    @staticmethod
    def show(text: str = "加载中..."):
        """显示加载动画"""
        spinner_html = f"""
        <div style="
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        ">
            <div style="
                border: 4px solid #f3f3f3;
                border-top: 4px solid #3498db;
                border-radius: 50%;
                width: 30px;
                height: 30px;
                animation: spin 1s linear infinite;
                margin-right: 10px;
            "></div>
            <span>{text}</span>
        </div>
        
        <style>
        @keyframes spin {{
            0% {{ transform: rotate(0deg); }}
            100% {{ transform: rotate(360deg); }}
        }}
        </style>
        """
        
        return st.markdown(spinner_html, unsafe_allow_html=True)

class ConfirmDialog:
    """确认对话框组件"""
    
    @staticmethod
    def show(message: str, confirm_key: str, 
             confirm_text: str = "确认", 
             cancel_text: str = "取消") -> Optional[bool]:
        """
        显示确认对话框
        
        Returns:
            True: 确认
            False: 取消
            None: 未操作
        """
        
        st.warning(message)
        
        col1, col2 = st.columns(2)
        
        with col1:
            if st.button(confirm_text, key=f"{confirm_key}_confirm", type="primary"):
                return True
        
        with col2:
            if st.button(cancel_text, key=f"{confirm_key}_cancel"):
                return False
        
        return None

class QuickActions:
    """快速操作组件"""
    
    @staticmethod
    def show(actions: List[Dict], columns: int = 4):
        """
        显示快速操作按钮
        
        Args:
            actions: 操作列表 [{'label': '添加服务', 'icon': '➕', 'callback': func}]
            columns: 列数
        """
        
        action_cols = st.columns(columns)
        
        for i, action in enumerate(actions):
            col_idx = i % columns
            
            with action_cols[col_idx]:
                button_text = f"{action.get('icon', '')} {action['label']}"
                
                if st.button(
                    button_text,
                    key=f"quick_action_{i}",
                    help=action.get('help', ''),
                    use_container_width=True
                ):
                    if 'callback' in action:
                        action['callback']()
                    elif 'key' in action:
                        st.session_state[action['key']] = True
