#!/usr/bin/env python3
"""
修复页面显示问题
"""

import sys
import os

# 添加当前目录到Python路径
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

def create_simple_app():
    """创建简化版应用"""
    print("🔧 创建简化版应用...")
    
    simple_app_content = '''
import streamlit as st
from utils.config_manager import SessionManager, WebConfigManager
from utils.api_client import MCPStoreAPI

# 页面配置
st.set_page_config(
    page_title="MCPStore 管理面板",
    page_icon="🚀",
    layout="wide",
    initial_sidebar_state="expanded"
)

def main():
    """主应用函数"""
    
    # 初始化会话状态
    if 'config_manager' not in st.session_state:
        st.session_state.config_manager = WebConfigManager()
    
    if 'api_client' not in st.session_state:
        st.session_state.api_client = MCPStoreAPI("http", "http://localhost:18611")
    
    if 'current_page' not in st.session_state:
        st.session_state.current_page = 'overview'
    
    # 应用CSS样式
    st.markdown("""
    <style>
    .main .block-container {
        padding-top: 2rem;
        padding-bottom: 2rem;
    }
    
    .css-1d391kg {
        background: linear-gradient(180deg, #f8f9fa 0%, #ffffff 100%);
        border-right: 1px solid #e9ecef;
    }
    
    .stButton > button {
        border-radius: 6px;
        margin-bottom: 0.25rem;
        font-weight: 500;
        transition: all 0.2s ease;
    }
    
    .stButton > button:hover {
        transform: translateX(2px);
        box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    }
    </style>
    """, unsafe_allow_html=True)
    
    # 状态栏
    render_status_bar()
    
    # 侧边栏
    with st.sidebar:
        render_sidebar()
    
    # 主内容
    render_main_content()

def render_status_bar():
    """渲染状态栏"""
    col1, col2, col3 = st.columns([2, 1, 1])
    
    with col1:
        st.markdown("""
        <div style="display: flex; align-items: center; padding: 0.5rem 1rem; background: #d4edda; border-radius: 6px; border-left: 4px solid #28a745;">
            <span style="color: #155724; font-weight: 500;">🟢 系统已连接</span>
        </div>
        """, unsafe_allow_html=True)
    
    with col2:
        import datetime
        current_time = datetime.datetime.now().strftime("%H:%M:%S")
        st.markdown(f"""
        <div style="text-align: center; padding: 0.5rem; color: #6c757d; font-size: 0.9rem;">
            🕐 {current_time}
        </div>
        """, unsafe_allow_html=True)
    
    with col3:
        if st.button("🔄 刷新", help="刷新所有数据", use_container_width=True):
            st.rerun()

def render_sidebar():
    """渲染侧边栏"""
    
    # 品牌标识
    st.markdown("""
    <div style="text-align: center; padding: 1rem 0;">
        <div style="font-size: 1.5rem; font-weight: bold; color: #1f77b4; margin-bottom: 0.5rem;">
            🚀 MCPStore
        </div>
        <div style="font-size: 0.9rem; color: #666; font-weight: 500;">
            管理控制台
        </div>
    </div>
    """, unsafe_allow_html=True)
    
    st.markdown("---")
    
    # 导航菜单
    st.markdown("### 功能模块")
    
    pages = [
        ("🏠", "系统概览", "overview"),
        ("🛠️", "服务管理", "service_management"),
        ("🔧", "工具管理", "tool_management"),
        ("👥", "Agent管理", "agent_management"),
        ("📊", "监控面板", "monitoring"),
        ("⚙️", "配置管理", "configuration")
    ]
    
    current_page = st.session_state.get('current_page', 'overview')
    
    for icon, name, page_key in pages:
        button_type = "primary" if current_page == page_key else "secondary"
        
        if st.button(f"{icon} {name}", key=f"nav_{page_key}", use_container_width=True, type=button_type):
            st.session_state.current_page = page_key
            st.rerun()
    
    st.markdown("---")
    
    # 系统状态
    st.markdown("### 系统状态")
    
    st.markdown("""
    <div style="background: white; border: 1px solid #e9ecef; border-radius: 8px; padding: 0.75rem; margin-bottom: 0.75rem;">
        <div style="display: flex; justify-content: space-between; align-items: center;">
            <span style="font-size: 0.8rem; font-weight: 600; color: #495057;">🏪 Store状态</span>
            <span style="font-size: 0.75rem; padding: 0.2rem 0.5rem; border-radius: 12px; background: #4CAF5020; color: #4CAF50; font-weight: 600;">正常</span>
        </div>
        <div style="font-size: 0.75rem; color: #6c757d; margin-top: 0.5rem;">服务: 1 | 健康: 1</div>
    </div>
    """, unsafe_allow_html=True)

def render_main_content():
    """渲染主内容"""
    
    current_page = st.session_state.get('current_page', 'overview')
    
    # 页面标题
    page_titles = {
        'overview': '🏠 系统概览',
        'service_management': '🛠️ 服务管理',
        'tool_management': '🔧 工具管理',
        'agent_management': '👥 Agent管理',
        'monitoring': '📊 监控面板',
        'configuration': '⚙️ 配置管理'
    }
    
    title = page_titles.get(current_page, '🏠 系统概览')
    
    st.markdown(f"""
    <div style="margin-bottom: 1.5rem;">
        <h2 style="color: #333; font-weight: 600; margin: 0; padding-bottom: 0.5rem; border-bottom: 2px solid #e9ecef;">
            {title}
        </h2>
    </div>
    """, unsafe_allow_html=True)
    
    # 页面内容
    if current_page == 'overview':
        show_overview()
    elif current_page == 'service_management':
        show_service_management()
    elif current_page == 'tool_management':
        show_tool_management()
    elif current_page == 'agent_management':
        show_agent_management()
    elif current_page == 'monitoring':
        show_monitoring()
    elif current_page == 'configuration':
        show_configuration()
    else:
        show_overview()

def show_overview():
    """显示系统概览"""
    st.markdown("## 欢迎使用 MCPStore 管理面板")
    st.info("这是一个简化版本，用于测试页面显示功能。")
    
    col1, col2, col3, col4 = st.columns(4)
    
    with col1:
        st.metric("服务总数", 1, "健康: 1")
    
    with col2:
        st.metric("工具总数", 5)
    
    with col3:
        st.metric("Agent数量", 0)
    
    with col4:
        st.metric("系统健康度", "100%", "良好")

def show_service_management():
    """显示服务管理"""
    st.info("服务管理页面 - 功能开发中")

def show_tool_management():
    """显示工具管理"""
    st.info("工具管理页面 - 功能开发中")

def show_agent_management():
    """显示Agent管理"""
    st.info("Agent管理页面 - 功能开发中")

def show_monitoring():
    """显示监控面板"""
    st.info("监控面板页面 - 功能开发中")

def show_configuration():
    """显示配置管理"""
    st.info("配置管理页面 - 功能开发中")

if __name__ == "__main__":
    main()
'''
    
    try:
        with open('app_simple.py', 'w', encoding='utf-8') as f:
            f.write(simple_app_content)
        print("✅ 简化版应用已创建: app_simple.py")
        return True
    except Exception as e:
        print(f"❌ 创建简化版应用失败: {e}")
        return False

def main():
    """主函数"""
    print("🔧 MCPStore 页面显示问题修复")
    print("=" * 40)
    
    if create_simple_app():
        print("\n✅ 简化版应用创建成功！")
        print("\n🚀 测试步骤:")
        print("1. 运行: streamlit run app_simple.py --server.port 8503")
        print("2. 访问: http://localhost:8503")
        print("3. 检查页面是否正常显示")
        print("\n💡 如果简化版正常，说明问题在复杂逻辑中")
        print("   如果简化版也有问题，说明是基础环境问题")
    else:
        print("❌ 创建简化版应用失败")

if __name__ == "__main__":
    main()
