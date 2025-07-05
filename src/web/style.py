"""
MCPStore Web界面样式定义
"""

import streamlit as st

def apply_custom_styles():
    """应用自定义样式"""
    
    custom_css = """
    <style>
    /* 主要样式 */
    .main {
        padding-top: 1rem;
    }
    
    /* 隐藏Streamlit默认元素 */
    #MainMenu {visibility: hidden;}
    footer {visibility: hidden;}
    header {visibility: hidden;}

    /* 隐藏Streamlit多页面导航 - 强力版本 */
    [data-testid="stSidebarNav"],
    [data-testid="stSidebarNavItems"],
    [data-testid="stSidebarNavLink"],
    .css-1544g2n,
    .css-1v0mbdj,
    .css-10trblm,
    .css-1rs6os,
    .css-17ziqus,
    .css-1vq4p4l,
    .css-pkbazv,
    .css-1y4p8pa,
    .css-1d391kg nav,
    section[data-testid="stSidebar"] nav,
    section[data-testid="stSidebar"] > div > div:first-child,
    .stSidebar nav {
        display: none !important;
        visibility: hidden !important;
        height: 0 !important;
        overflow: hidden !important;
    }

    /* 隐藏侧边栏中的页面链接 */
    .css-1d391kg a[href*="app"],
    .css-1d391kg a[href*="agent_management"],
    .css-1d391kg a[href*="configuration"],
    .css-1d391kg a[href*="monitoring"],
    .css-1d391kg a[href*="service_management"],
    .css-1d391kg a[href*="tool_management"] {
        display: none !important;
    }

    /* 确保侧边栏内容从顶部开始 */
    .css-1d391kg {
        padding-top: 1rem !important;
    }

    /* 隐藏任何包含页面名称的元素 */
    .css-1d391kg div:contains("app"),
    .css-1d391kg div:contains("agent management"),
    .css-1d391kg div:contains("configuration"),
    .css-1d391kg div:contains("monitoring"),
    .css-1d391kg div:contains("service management"),
    .css-1d391kg div:contains("tool management") {
        display: none !important;
    }
    
    /* 自定义按钮样式 */
    .stButton > button {
        border-radius: 8px;
        border: 1px solid #ddd;
        transition: all 0.3s ease;
    }
    
    .stButton > button:hover {
        transform: translateY(-2px);
        box-shadow: 0 4px 8px rgba(0,0,0,0.1);
    }
    
    /* 指标卡片样式 */
    .metric-card {
        background: white;
        padding: 1rem;
        border-radius: 8px;
        border: 1px solid #e0e0e0;
        box-shadow: 0 2px 4px rgba(0,0,0,0.05);
        margin: 0.5rem 0;
    }
    
    /* 状态指示器样式 */
    .status-indicator {
        display: inline-flex;
        align-items: center;
        padding: 0.25rem 0.5rem;
        border-radius: 12px;
        font-size: 0.875rem;
        font-weight: 500;
    }
    
    .status-healthy {
        background-color: #d4edda;
        color: #155724;
    }
    
    .status-unhealthy {
        background-color: #f8d7da;
        color: #721c24;
    }
    
    .status-unknown {
        background-color: #fff3cd;
        color: #856404;
    }
    
    /* 服务卡片样式 */
    .service-card {
        background: white;
        border: 1px solid #e0e0e0;
        border-radius: 8px;
        padding: 1rem;
        margin: 0.5rem 0;
        transition: all 0.3s ease;
    }
    
    .service-card:hover {
        box-shadow: 0 4px 12px rgba(0,0,0,0.1);
        border-color: #1f77b4;
    }
    
    /* 标签页样式优化 */
    .stTabs [data-baseweb="tab-list"] {
        gap: 8px;
    }
    
    .stTabs [data-baseweb="tab"] {
        height: 50px;
        padding-left: 20px;
        padding-right: 20px;
        border-radius: 8px 8px 0 0;
        background-color: #f8f9fa;
        border: 1px solid #dee2e6;
        border-bottom: none;
    }
    
    .stTabs [aria-selected="true"] {
        background-color: white;
        border-color: #1f77b4;
        color: #1f77b4;
    }
    
    /* 侧边栏样式优化 - 专业外观 */
    .css-1d391kg {
        background: #ffffff;
        border-right: 1px solid #e9ecef;
        box-shadow: 2px 0 4px rgba(0,0,0,0.02);
        padding-top: 0 !important;
    }

    /* 侧边栏内容区域优化 */
    .css-1d391kg .element-container {
        margin-bottom: 0.25rem;
    }

    /* 侧边栏按钮样式优化 - 专业导航按钮 */
    .css-1d391kg .stButton {
        margin-bottom: 1px;
    }

    .css-1d391kg .stButton > button {
        width: 100%;
        border-radius: 6px;
        border: none;
        background-color: transparent;
        color: #6c757d;
        font-weight: 500;
        font-size: 0.875rem;
        padding: 0.6rem 1rem;
        text-align: left;
        transition: all 0.2s ease;
        box-shadow: none;
        min-height: auto;
        height: auto;
    }

    .css-1d391kg .stButton > button:hover {
        background-color: #f8f9fa;
        color: #495057;
        transform: none;
        box-shadow: none;
    }

    /* 选中状态的按钮 - Primary */
    .css-1d391kg .stButton > button[kind="primary"] {
        background-color: #f8f9fa;
        border-left: 3px solid #007bff;
        color: #007bff;
        font-weight: 600;
    }

    .css-1d391kg .stButton > button[kind="primary"]:hover {
        background-color: #e9ecef;
        color: #0056b3;
    }

    /* 侧边栏整体间距优化 */
    .css-1d391kg .block-container {
        padding-top: 1rem;
        padding-bottom: 1rem;
    }

    /* 侧边栏导航项样式 - 专业设计 */
    .css-1d391kg div[style*="cursor: pointer"] {
        margin-bottom: 2px;
        border-radius: 6px;
        transition: all 0.2s ease;
        background: transparent;
    }

    .css-1d391kg div[style*="cursor: pointer"]:hover {
        background: #f8f9fa !important;
        transform: none;
    }

    /* 当前选中的导航项 */
    .css-1d391kg div[style*="background: #f8f9fa"] {
        background: #f8f9fa !important;
        border-left: 3px solid #007bff !important;
    }

    /* 侧边栏滚动条样式 */
    .css-1d391kg::-webkit-scrollbar {
        width: 4px;
    }

    .css-1d391kg::-webkit-scrollbar-track {
        background: transparent;
    }

    .css-1d391kg::-webkit-scrollbar-thumb {
        background: #dee2e6;
        border-radius: 2px;
    }

    .css-1d391kg::-webkit-scrollbar-thumb:hover {
        background: #adb5bd;
    }

    /* 主内容区域样式 */
    .main .block-container {
        padding-top: 2rem;
        padding-bottom: 2rem;
        max-width: none;
    }

    /* 页面标题样式 */
    .main h1, .main h2, .main h3 {
        font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
    }

    /* 卡片样式 */
    .metric-card, .status-card {
        background: white;
        border-radius: 12px;
        padding: 1.5rem;
        box-shadow: 0 2px 10px rgba(0,0,0,0.08);
        border: 1px solid #e9ecef;
        transition: all 0.3s ease;
    }

    .metric-card:hover, .status-card:hover {
        box-shadow: 0 4px 20px rgba(0,0,0,0.12);
        transform: translateY(-2px);
    }

    /* 加载动画优化 */
    .loading-container {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        min-height: 300px;
        text-align: center;
    }

    .loading-spinner {
        width: 50px;
        height: 50px;
        border: 4px solid #f3f3f3;
        border-top: 4px solid #1f77b4;
        border-radius: 50%;
        animation: spin 1s linear infinite;
        margin-bottom: 1rem;
    }

    @keyframes spin {
        0% { transform: rotate(0deg); }
        100% { transform: rotate(360deg); }
    }

    /* 状态指示器优化 */
    .status-indicator {
        display: inline-flex;
        align-items: center;
        padding: 0.4rem 0.8rem;
        border-radius: 20px;
        font-size: 0.85rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }

    .status-healthy {
        background: linear-gradient(135deg, #d4edda, #c3e6cb);
        color: #155724;
        border: 1px solid #c3e6cb;
    }

    .status-unhealthy {
        background: linear-gradient(135deg, #f8d7da, #f5c6cb);
        color: #721c24;
        border: 1px solid #f5c6cb;
    }

    .status-warning {
        background: linear-gradient(135deg, #fff3cd, #ffeaa7);
        color: #856404;
        border: 1px solid #ffeaa7;
    }
    
    /* 通知样式 */
    .notification {
        padding: 0.75rem 1rem;
        border-radius: 6px;
        margin: 0.5rem 0;
        border-left: 4px solid;
        animation: slideIn 0.3s ease-out;
    }
    
    .notification-success {
        background-color: #d4edda;
        border-left-color: #28a745;
        color: #155724;
    }
    
    .notification-error {
        background-color: #f8d7da;
        border-left-color: #dc3545;
        color: #721c24;
    }
    
    .notification-warning {
        background-color: #fff3cd;
        border-left-color: #ffc107;
        color: #856404;
    }
    
    .notification-info {
        background-color: #d1ecf1;
        border-left-color: #17a2b8;
        color: #0c5460;
    }
    
    @keyframes slideIn {
        from {
            opacity: 0;
            transform: translateX(-100%);
        }
        to {
            opacity: 1;
            transform: translateX(0);
        }
    }
    
    /* 模态窗口样式 */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background-color: rgba(0, 0, 0, 0.5);
        z-index: 1000;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    
    .modal-content {
        background: white;
        border-radius: 12px;
        padding: 2rem;
        max-width: 600px;
        width: 90%;
        max-height: 80vh;
        overflow-y: auto;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    }
    
    /* 响应式设计 */
    @media (max-width: 768px) {
        .modal-content {
            width: 95%;
            padding: 1rem;
        }
        
        .stButton > button {
            width: 100%;
        }
    }
    
    /* 加载动画 */
    .loading-spinner {
        border: 3px solid #f3f3f3;
        border-top: 3px solid #1f77b4;
        border-radius: 50%;
        width: 30px;
        height: 30px;
        animation: spin 1s linear infinite;
        margin: 0 auto;
    }
    
    @keyframes spin {
        0% { transform: rotate(0deg); }
        100% { transform: rotate(360deg); }
    }
    
    /* 工具提示样式 */
    .tooltip {
        position: relative;
        display: inline-block;
    }
    
    .tooltip .tooltiptext {
        visibility: hidden;
        width: 200px;
        background-color: #333;
        color: white;
        text-align: center;
        border-radius: 6px;
        padding: 5px;
        position: absolute;
        z-index: 1;
        bottom: 125%;
        left: 50%;
        margin-left: -100px;
        opacity: 0;
        transition: opacity 0.3s;
    }
    
    .tooltip:hover .tooltiptext {
        visibility: visible;
        opacity: 1;
    }
    
    /* 表格样式优化 */
    .dataframe {
        border: none !important;
    }
    
    .dataframe th {
        background-color: #f8f9fa !important;
        border: 1px solid #dee2e6 !important;
        padding: 0.75rem !important;
    }
    
    .dataframe td {
        border: 1px solid #dee2e6 !important;
        padding: 0.75rem !important;
    }
    
    /* 代码块样式 */
    .stCode {
        border-radius: 8px;
        border: 1px solid #e0e0e0;
    }
    
    /* 成功/错误消息样式 */
    .stSuccess {
        border-radius: 8px;
        border-left: 4px solid #28a745;
    }
    
    .stError {
        border-radius: 8px;
        border-left: 4px solid #dc3545;
    }
    
    .stWarning {
        border-radius: 8px;
        border-left: 4px solid #ffc107;
    }
    
    .stInfo {
        border-radius: 8px;
        border-left: 4px solid #17a2b8;
    }
    </style>
    """
    
    st.markdown(custom_css, unsafe_allow_html=True)

    # 添加JavaScript来动态隐藏页面导航
    hide_navigation_js = """
    <script>
    function hideNavigation() {
        // 隐藏侧边栏中的页面导航
        const sidebar = document.querySelector('[data-testid="stSidebar"]');
        if (sidebar) {
            // 查找并隐藏所有包含页面名称的链接
            const pageLinks = sidebar.querySelectorAll('a');
            pageLinks.forEach(link => {
                const text = link.textContent.toLowerCase();
                if (text.includes('app') ||
                    text.includes('agent management') ||
                    text.includes('configuration') ||
                    text.includes('monitoring') ||
                    text.includes('service management') ||
                    text.includes('tool management')) {
                    link.style.display = 'none';
                    // 也隐藏父容器
                    if (link.parentElement) {
                        link.parentElement.style.display = 'none';
                    }
                }
            });

            // 隐藏导航容器
            const navElements = sidebar.querySelectorAll('nav, [data-testid="stSidebarNav"]');
            navElements.forEach(nav => {
                nav.style.display = 'none';
            });
        }
    }

    // 页面加载后执行
    document.addEventListener('DOMContentLoaded', hideNavigation);

    // 定期检查并隐藏（因为Streamlit可能动态添加元素）
    setInterval(hideNavigation, 1000);
    </script>
    """

    st.markdown(hide_navigation_js, unsafe_allow_html=True)

def create_status_badge(status: str, text: str = None) -> str:
    """创建状态徽章HTML"""
    status_classes = {
        'healthy': 'status-healthy',
        'unhealthy': 'status-unhealthy', 
        'unknown': 'status-unknown'
    }
    
    css_class = status_classes.get(status, 'status-unknown')
    display_text = text or status
    
    return f'<span class="status-indicator {css_class}">{display_text}</span>'

def create_notification_html(message: str, type: str = "info") -> str:
    """创建通知HTML"""
    return f'''
    <div class="notification notification-{type}">
        {message}
    </div>
    '''

def create_loading_spinner() -> str:
    """创建加载动画HTML"""
    return '''
    <div style="display: flex; justify-content: center; align-items: center; padding: 2rem;">
        <div class="loading-spinner"></div>
        <span style="margin-left: 1rem;">加载中...</span>
    </div>
    '''
