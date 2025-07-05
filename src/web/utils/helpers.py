"""
辅助函数和工具
"""

import streamlit as st
from datetime import datetime
from typing import Dict, List, Any, Optional
import json
import re

from .api_client import MCPStoreAPI

def init_session_state():
    """初始化会话状态"""
    if 'api_base' not in st.session_state:
        st.session_state.api_base = 'http://localhost:18611'
    
    if 'api_client' not in st.session_state:
        st.session_state.api_client = MCPStoreAPI(st.session_state.api_base)
    
    if 'agents' not in st.session_state:
        st.session_state.agents = []
    
    if 'selected_agent' not in st.session_state:
        st.session_state.selected_agent = None
    
    if 'last_refresh' not in st.session_state:
        st.session_state.last_refresh = datetime.now()

def format_json(data: Dict) -> str:
    """格式化JSON数据"""
    return json.dumps(data, indent=2, ensure_ascii=False)

def validate_url(url: str) -> bool:
    """验证URL格式"""
    url_pattern = re.compile(
        r'^https?://'  # http:// or https://
        r'(?:(?:[A-Z0-9](?:[A-Z0-9-]{0,61}[A-Z0-9])?\.)+[A-Z]{2,6}\.?|'  # domain...
        r'localhost|'  # localhost...
        r'\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})'  # ...or ip
        r'(?::\d+)?'  # optional port
        r'(?:/?|[/?]\S+)$', re.IGNORECASE)
    return url_pattern.match(url) is not None

def validate_service_name(name: str) -> bool:
    """验证服务名称"""
    if not name or len(name.strip()) == 0:
        return False
    
    # 检查是否包含特殊字符
    if re.search(r'[<>:"/\\|?*]', name):
        return False
    
    return True

def get_status_color(status: str) -> str:
    """根据状态获取颜色"""
    status_colors = {
        'healthy': '🟢',
        'unhealthy': '🔴', 
        'unknown': '🟡',
        'connecting': '🟠',
        'disconnected': '⚫'
    }
    return status_colors.get(status.lower(), '🟡')

def get_status_text(status: str) -> str:
    """根据状态获取文本"""
    status_texts = {
        'healthy': '健康',
        'unhealthy': '异常',
        'unknown': '未知',
        'connecting': '连接中',
        'disconnected': '已断开'
    }
    return status_texts.get(status.lower(), '未知')

def show_success_message(message: str):
    """显示成功消息"""
    st.success(f"✅ {message}")

def show_error_message(message: str):
    """显示错误消息"""
    st.error(f"❌ {message}")

def show_warning_message(message: str):
    """显示警告消息"""
    st.warning(f"⚠️ {message}")

def show_info_message(message: str):
    """显示信息消息"""
    st.info(f"ℹ️ {message}")

def create_service_card(service: Dict) -> None:
    """创建服务卡片"""
    with st.container():
        col1, col2, col3 = st.columns([3, 1, 1])
        
        with col1:
            status_icon = get_status_color(service.get('status', 'unknown'))
            st.markdown(f"**{status_icon} {service.get('name', 'Unknown')}**")
            st.caption(service.get('url', 'No URL'))
        
        with col2:
            tool_count = service.get('tool_count', 0)
            st.metric("工具数", tool_count)
        
        with col3:
            if st.button("详情", key=f"detail_{service.get('name')}"):
                st.session_state.selected_service = service.get('name')

def create_tool_card(tool: Dict) -> None:
    """创建工具卡片"""
    with st.container():
        st.markdown(f"**🔧 {tool.get('name', 'Unknown')}**")
        st.caption(tool.get('description', 'No description'))
        
        if st.button("测试", key=f"test_{tool.get('name')}"):
            st.session_state.selected_tool = tool.get('name')

def create_agent_card(agent_id: str, agent_data: Dict) -> None:
    """创建Agent卡片"""
    with st.container():
        col1, col2, col3 = st.columns([2, 1, 1])
        
        with col1:
            st.markdown(f"**👤 {agent_id}**")
            st.caption(f"服务数: {agent_data.get('service_count', 0)}")
        
        with col2:
            tool_count = agent_data.get('tool_count', 0)
            st.metric("工具数", tool_count)
        
        with col3:
            if st.button("管理", key=f"manage_{agent_id}"):
                st.session_state.selected_agent = agent_id

def parse_tool_schema(schema: Dict) -> Dict:
    """解析工具参数schema"""
    if not schema or 'properties' not in schema:
        return {}
    
    return schema['properties']

def create_dynamic_form(tool_name: str, schema: Dict) -> Dict:
    """根据schema创建动态表单"""
    st.subheader(f"🔧 测试工具: {tool_name}")
    
    form_data = {}
    properties = parse_tool_schema(schema)
    
    if not properties:
        st.info("此工具无需参数")
        return {}
    
    with st.form(f"tool_form_{tool_name}"):
        for param_name, param_info in properties.items():
            param_type = param_info.get('type', 'string')
            param_desc = param_info.get('description', '')
            required = param_name in schema.get('required', [])
            
            label = f"{param_name}"
            if required:
                label += " *"
            
            if param_type == 'string':
                form_data[param_name] = st.text_input(
                    label, 
                    help=param_desc,
                    key=f"{tool_name}_{param_name}"
                )
            elif param_type == 'integer':
                form_data[param_name] = st.number_input(
                    label,
                    help=param_desc,
                    step=1,
                    key=f"{tool_name}_{param_name}"
                )
            elif param_type == 'number':
                form_data[param_name] = st.number_input(
                    label,
                    help=param_desc,
                    key=f"{tool_name}_{param_name}"
                )
            elif param_type == 'boolean':
                form_data[param_name] = st.checkbox(
                    label,
                    help=param_desc,
                    key=f"{tool_name}_{param_name}"
                )
            else:
                form_data[param_name] = st.text_input(
                    label,
                    help=f"{param_desc} (类型: {param_type})",
                    key=f"{tool_name}_{param_name}"
                )
        
        submitted = st.form_submit_button("🚀 执行工具")
        
        if submitted:
            # 验证必需参数
            missing_params = []
            for param_name in schema.get('required', []):
                if not form_data.get(param_name):
                    missing_params.append(param_name)
            
            if missing_params:
                show_error_message(f"缺少必需参数: {', '.join(missing_params)}")
                return None
            
            # 清理空值
            cleaned_data = {k: v for k, v in form_data.items() if v is not None and v != ''}
            return cleaned_data
    
    return None

def format_tool_result(result: Any) -> str:
    """格式化工具执行结果"""
    if isinstance(result, dict):
        return format_json(result)
    elif isinstance(result, list):
        return format_json(result)
    else:
        return str(result)

def get_preset_services() -> List[Dict]:
    """获取预设服务列表"""
    return [
        {
            "name": "mcpstore-wiki",
            "url": "http://59.110.160.18:21923/mcp",
            "description": "MCPStore官方Wiki服务"
        },
        {
            "name": "mcpstore-demo", 
            "url": "http://59.110.160.18:21924/mcp",
            "description": "MCPStore演示服务"
        }
    ]

def export_config(config: Dict) -> str:
    """导出配置为JSON字符串"""
    return format_json(config)

def import_config(config_str: str) -> Optional[Dict]:
    """从JSON字符串导入配置"""
    try:
        return json.loads(config_str)
    except json.JSONDecodeError as e:
        show_error_message(f"配置格式错误: {e}")
        return None
