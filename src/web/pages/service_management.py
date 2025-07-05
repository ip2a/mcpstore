"""
服务管理页面
"""

import streamlit as st
from typing import Dict, List
import json

from utils.helpers import (
    show_success_message, show_error_message, show_warning_message,
    validate_url, validate_service_name, create_service_card,
    get_status_color, get_status_text, get_preset_services,
    format_json
)

def show():
    """显示服务管理页面"""
    st.header("🛠️ 服务管理")

    # 创建标签页
    tab1, tab2, tab3 = st.tabs(["📋 服务列表", "➕ 添加服务", "🔧 服务详情"])

    with tab1:
        show_service_list()

    with tab2:
        show_add_service()

    with tab3:
        show_service_details()

def show_service_list():
    """显示服务列表"""
    st.subheader("📋 已注册服务")
    
    # 操作按钮
    col1, col2, col3, col4 = st.columns([1, 1, 1, 1])

    with col1:
        if st.button("🔄 刷新列表", key="service_refresh_list"):
            st.rerun()

    with col2:
        if st.button("🔍 检查健康", key="service_check_health"):
            check_all_services_health()

    with col3:
        show_batch_operations = st.button("📦 批量操作", key="toggle_batch_operations")
        if show_batch_operations:
            st.session_state.show_batch_ops = not st.session_state.get('show_batch_ops', False)
    
    # 获取服务列表
    api_client = st.session_state.api_client
    # 对应API: GET /for_store/list_services
    # 实际调用: store.for_store().list_services()
    response = api_client.list_services()
    
    if not response:
        show_error_message("无法获取服务列表")
        return
    
    services = response.get('data', [])
    
    if not services:
        st.info("暂无已注册的服务")
        return
    
    # 显示服务统计
    healthy_count = sum(1 for s in services if s.get('status') == 'healthy')
    st.metric("服务统计", f"{len(services)} 个服务", f"{healthy_count} 个健康")

    # 批量操作面板
    if st.session_state.get('show_batch_ops', False):
        show_batch_operations_panel(services)
    
    # 服务列表
    for service in services:
        with st.container():
            col1, col2, col3, col4, col5 = st.columns([3, 1, 1, 1, 2])
            
            with col1:
                status_icon = get_status_color(service.get('status', 'unknown'))
                st.markdown(f"**{status_icon} {service.get('name', 'Unknown')}**")
                st.caption(service.get('url', 'No URL'))
            
            with col2:
                tool_count = service.get('tool_count', 0)
                st.metric("工具", tool_count)
            
            with col3:
                status_text = get_status_text(service.get('status', 'unknown'))
                st.write(status_text)
            
            with col4:
                if st.button("📊 详情", key=f"detail_{service.get('name')}"):
                    st.session_state.selected_service = service.get('name')
                    st.rerun()
            
            with col5:
                # 操作按钮
                col5_1, col5_2, col5_3 = st.columns(3)
                
                with col5_1:
                    if st.button("🔄", key=f"restart_{service.get('name')}", help="重启服务"):
                        restart_service(service.get('name'))
                
                with col5_2:
                    if st.button("✏️", key=f"edit_{service.get('name')}", help="编辑服务"):
                        st.session_state.edit_service = service.get('name')
                        st.rerun()
                
                with col5_3:
                    if st.button("🗑️", key=f"delete_{service.get('name')}", help="删除服务"):
                        delete_service(service.get('name'))
            
            st.markdown("---")

def show_add_service():
    """显示添加服务页面"""
    st.subheader("➕ 添加新服务")

    # 创建添加方式选择
    add_method = st.radio(
        "选择添加方式",
        ["📄 根据MCP配置文件注册", "📝 表单填写单个服务", "📋 JSON配置单个服务", "📦 批量添加服务"],
        horizontal=True
    )

    st.markdown("---")

    if add_method == "📄 根据MCP配置文件注册":
        show_add_from_mcpconfig()
    elif add_method == "📝 表单填写单个服务":
        show_add_single_form()
    elif add_method == "📋 JSON配置单个服务":
        show_add_single_json()
    elif add_method == "📦 批量添加服务":
        show_add_batch()

def show_add_from_mcpconfig():
    """根据MCP配置文件注册服务"""
    st.markdown("#### 📄 根据MCP配置文件注册服务")
    st.info("此功能将读取Store的MCP配置文件，并将其中的服务注册到当前Store中")

    col1, col2 = st.columns([2, 1])

    with col1:
        st.markdown("**操作说明**:")
        st.markdown("1. 确保您的MCP配置文件已正确配置")
        st.markdown("2. 点击下方按钮读取配置文件中的服务")
        st.markdown("3. 选择要注册的服务")
        st.markdown("4. 确认注册")

    with col2:
        if st.button("📖 读取MCP配置", key="read_mcp_config", type="primary"):
            read_and_register_from_mcpconfig()

    # 显示从MCP配置读取的服务选择界面
    if 'mcp_services_to_register' in st.session_state:
        show_mcp_services_selection()

def show_add_single_form():
    """表单填写单个服务"""
    st.markdown("#### 📝 表单填写单个服务")

    with st.form("add_single_service_form"):
        col1, col2 = st.columns(2)

        with col1:
            service_name = st.text_input(
                "服务名称 *",
                help="服务的唯一标识符，只能包含字母、数字、下划线和连字符"
            )

            service_url = st.text_input(
                "服务URL *",
                placeholder="http://example.com/mcp",
                help="MCP服务的完整URL地址"
            )

            transport_type = st.selectbox(
                "传输类型",
                ["auto", "sse", "streamable-http"],
                help="选择auto将根据URL自动推断传输类型"
            )

        with col2:
            description = st.text_area(
                "服务描述",
                placeholder="描述此服务的功能和用途",
                help="可选的服务描述信息"
            )

            keep_alive = st.checkbox(
                "保持连接",
                value=False,
                help="是否保持长连接"
            )

            timeout = st.number_input(
                "超时时间(秒)",
                min_value=1,
                max_value=300,
                value=30,
                help="请求超时时间"
            )

        # 高级选项
        with st.expander("🔧 高级选项"):
            headers_text = st.text_area(
                "请求头 (JSON格式)",
                placeholder='{"Authorization": "Bearer token", "Content-Type": "application/json"}',
                help="自定义HTTP请求头"
            )

            env_text = st.text_area(
                "环境变量 (JSON格式)",
                placeholder='{"API_KEY": "your_key", "DEBUG": "true"}',
                help="服务运行时的环境变量"
            )

        submitted = st.form_submit_button("🚀 添加服务", type="primary")

        if submitted:
            add_service_from_form(service_name, service_url, transport_type, description,
                                keep_alive, timeout, headers_text, env_text)

def show_add_single_json():
    """JSON配置单个服务"""
    st.markdown("#### 📋 JSON配置单个服务")

    col1, col2 = st.columns([2, 1])

    with col1:
        st.markdown("**JSON配置格式**:")
        example_config = {
            "name": "example_service",
            "url": "http://example.com/mcp",
            "transport": "auto",
            "description": "示例服务",
            "timeout": 30,
            "keep_alive": False,
            "headers": {
                "Authorization": "Bearer token"
            },
            "env": {
                "API_KEY": "your_key"
            }
        }

        json_config = st.text_area(
            "服务配置 (JSON格式)",
            value=json.dumps(example_config, indent=2, ensure_ascii=False),
            height=300,
            help="请按照示例格式填写服务配置"
        )

        if st.button("🚀 添加服务", key="add_single_json", type="primary"):
            add_service_from_json(json_config)

    with col2:
        st.markdown("**必填字段**:")
        st.markdown("• `name`: 服务名称")
        st.markdown("• `url`: 服务URL")

        st.markdown("**可选字段**:")
        st.markdown("• `transport`: 传输类型")
        st.markdown("• `description`: 服务描述")
        st.markdown("• `timeout`: 超时时间")
        st.markdown("• `keep_alive`: 保持连接")
        st.markdown("• `headers`: 请求头")
        st.markdown("• `env`: 环境变量")

def show_add_batch():
    """批量添加服务"""
    st.markdown("#### 📦 批量添加服务")

    st.markdown("**JSON数组格式**:")
    example_batch = [
        {
            "name": "service1",
            "url": "http://example1.com/mcp",
            "description": "第一个服务"
        },
        {
            "name": "service2",
            "url": "http://example2.com/mcp",
            "transport": "sse",
            "description": "第二个服务"
        }
    ]

    json_config = st.text_area(
        "批量服务配置 (JSON数组格式)",
        value=json.dumps(example_batch, indent=2, ensure_ascii=False),
        height=400,
        help="请按照示例格式填写多个服务配置"
    )

    col1, col2, col3 = st.columns([1, 1, 2])

    with col1:
        if st.button("🚀 批量添加", key="batch_add_services", type="primary"):
            batch_add_from_json(json_config)

    with col2:
        if st.button("✅ 验证配置", key="validate_batch_config"):
            validate_batch_config(json_config)

    # 显示配置说明
    with st.expander("📖 配置说明"):
        st.markdown("""
        **批量添加规则**:
        - 每个服务必须包含 `name` 和 `url` 字段
        - 服务名称必须唯一
        - 如果某个服务添加失败，其他服务仍会继续添加
        - 添加完成后会显示详细的成功/失败统计

        **支持的字段**:
        - `name`: 服务名称 (必填)
        - `url`: 服务URL (必填)
        - `transport`: 传输类型 (可选: auto/sse/streamable-http)
        - `description`: 服务描述 (可选)
        - `timeout`: 超时时间 (可选)
        - `keep_alive`: 保持连接 (可选)
        - `headers`: 请求头 (可选)
        - `env`: 环境变量 (可选)
        """)



def show_service_details():
    """显示服务详情页面"""
    selected_service = st.session_state.get('selected_service')

    if not selected_service:
        st.info("💡 请从服务列表中点击 '📊 详情' 按钮查看服务详情")

        # 显示服务选择器
        api_client = st.session_state.api_client
        # 对应API: GET /for_store/list_services
        # 实际调用: store.for_store().list_services()
        response = api_client.list_services()

        if response and response.get('data'):
            services = response['data']
            service_names = [s.get('name') for s in services]

            if service_names:
                st.markdown("#### 🔍 或者直接选择服务:")
                selected = st.selectbox(
                    "选择要查看的服务",
                    [""] + service_names,
                    key="service_selector"
                )

                if selected:
                    st.session_state.selected_service = selected
                    st.rerun()

        return

    st.subheader(f"🔧 服务详情: {selected_service}")

    # 获取服务详细信息
    api_client = st.session_state.api_client
    response = api_client.get_service_info(selected_service)

    if not response:
        show_error_message("无法获取服务详情")
        return

    service_data = response.get('data', {})
    service_info = service_data.get('service', {})
    tools = service_data.get('tools', [])
    connected = service_data.get('connected', False)

    # 顶部操作栏
    col1, col2, col3, col4, col5 = st.columns([1, 1, 1, 1, 1])

    with col1:
        if st.button("🔄 重启", key="detail_restart_service", help="重启服务"):
            restart_service(selected_service)

    with col2:
        if st.button("✏️ 编辑", key="detail_edit_service", help="编辑服务配置"):
            st.session_state.edit_service_detail = selected_service
            st.rerun()

    with col3:
        if st.button("📊 状态", key="detail_get_status", help="获取详细状态"):
            get_service_status(selected_service)

    with col4:
        if st.button("🗑️ 删除", key="detail_delete_service", help="删除服务"):
            delete_service(selected_service)

    with col5:
        if st.button("🔙 返回", key="detail_back", help="返回服务列表"):
            if 'selected_service' in st.session_state:
                del st.session_state['selected_service']
            st.rerun()

    st.markdown("---")

    # 显示服务编辑表单
    if st.session_state.get('edit_service_detail') == selected_service:
        show_service_edit_form(selected_service, service_info)
        return

    # 服务概览卡片
    with st.container():
        # 状态指示器
        status_color = "🟢" if connected else "🔴"
        status_text = "已连接" if connected else "未连接"

        col1, col2, col3 = st.columns([2, 1, 1])

        with col1:
            st.markdown(f"### {status_color} {service_info.get('name', 'Unknown')}")
            st.markdown(f"**URL**: `{service_info.get('url', 'N/A')}`")
            st.markdown(f"**状态**: {status_color} {status_text}")

        with col2:
            st.metric("🔧 工具数量", len(tools))
            st.metric("🚀 传输类型", service_info.get('transport', 'auto'))

        with col3:
            # 健康状态
            if connected:
                st.success("服务正常运行")
            else:
                st.error("服务连接异常")

            # 最后检查时间
            import datetime
            st.caption(f"检查时间: {datetime.datetime.now().strftime('%H:%M:%S')}")

    st.markdown("---")

    # 详细信息标签页
    info_tab1, info_tab2, info_tab3 = st.tabs(["📋 基本信息", "🔧 工具列表", "⚙️ 配置详情"])

    with info_tab1:
        show_service_basic_info(service_info, service_data)

    with info_tab2:
        show_service_tools(tools, selected_service)

    with info_tab3:
        show_service_config_details(service_info)

def show_service_basic_info(service_info: Dict, service_data: Dict):
    """显示服务基本信息"""
    col1, col2 = st.columns(2)

    with col1:
        st.markdown("#### 📋 服务信息")

        info_items = [
            ("服务名称", service_info.get('name', 'N/A')),
            ("服务URL", service_info.get('url', 'N/A')),
            ("传输类型", service_info.get('transport', 'auto')),
            ("连接状态", "已连接" if service_data.get('connected') else "未连接"),
            ("服务描述", service_info.get('description', '无描述'))
        ]

        for label, value in info_items:
            st.write(f"**{label}**: {value}")

    with col2:
        st.markdown("#### 📊 运行统计")

        # 模拟一些统计信息
        tools_count = len(service_data.get('tools', []))
        st.metric("可用工具", tools_count)

        if service_info.get('timeout'):
            st.metric("超时设置", f"{service_info['timeout']}秒")

        if service_info.get('keep_alive'):
            st.info("✅ 启用长连接")
        else:
            st.info("❌ 未启用长连接")

def show_service_tools(tools: List[Dict], service_name: str):
    """显示服务工具列表"""
    if not tools:
        st.info("🔧 此服务暂无可用工具")
        return

    st.markdown(f"#### 🔧 可用工具 ({len(tools)} 个)")

    # 工具搜索
    if len(tools) > 5:
        search_term = st.text_input("🔍 搜索工具", placeholder="输入工具名称或描述关键词")
        if search_term:
            tools = [t for t in tools if search_term.lower() in t.get('name', '').lower()
                    or search_term.lower() in t.get('description', '').lower()]

    # 工具列表
    for i, tool in enumerate(tools):
        tool_name = tool.get('name', f'Tool_{i}')
        tool_desc = tool.get('description', '无描述')

        with st.expander(f"🔧 {tool_name}", expanded=False):
            col1, col2 = st.columns([2, 1])

            with col1:
                st.markdown(f"**描述**: {tool_desc}")

                # 显示参数schema
                if 'inputSchema' in tool:
                    st.markdown("**参数结构**:")
                    schema = tool['inputSchema']

                    # 简化显示
                    if 'properties' in schema:
                        st.markdown("**参数列表**:")
                        for prop_name, prop_info in schema['properties'].items():
                            prop_type = prop_info.get('type', 'unknown')
                            prop_desc = prop_info.get('description', '无描述')
                            required = prop_name in schema.get('required', [])
                            required_mark = " *" if required else ""
                            st.write(f"• `{prop_name}` ({prop_type}){required_mark}: {prop_desc}")

                    # 完整schema
                    with st.expander("查看完整Schema"):
                        st.code(format_json(schema), language='json')

            with col2:
                st.markdown("**操作**:")
                if st.button(f"🧪 测试", key=f"test_tool_{tool_name}_{service_name}"):
                    st.session_state.test_tool_name = tool_name
                    st.session_state.test_tool_schema = tool.get('inputSchema', {})
                    st.session_state.test_service_name = service_name
                    st.success(f"已选择工具 {tool_name} 进行测试，请前往工具管理页面")

def show_service_config_details(service_info: Dict):
    """显示服务配置详情"""
    st.markdown("#### ⚙️ 配置详情")

    # 基础配置
    with st.expander("🔧 基础配置", expanded=True):
        config_data = {
            "name": service_info.get('name'),
            "url": service_info.get('url'),
            "transport": service_info.get('transport', 'auto'),
            "description": service_info.get('description', ''),
            "timeout": service_info.get('timeout', 30),
            "keep_alive": service_info.get('keep_alive', False)
        }

        st.code(format_json(config_data), language='json')

    # 高级配置
    if service_info.get('headers') or service_info.get('env'):
        with st.expander("🔧 高级配置"):
            if service_info.get('headers'):
                st.markdown("**请求头**:")
                st.code(format_json(service_info['headers']), language='json')

            if service_info.get('env'):
                st.markdown("**环境变量**:")
                st.code(format_json(service_info['env']), language='json')

    # 完整配置
    with st.expander("📄 完整配置 (JSON)"):
        st.code(format_json(service_info), language='json')

def show_batch_operations_panel(services: List[Dict]):
    """显示批量操作面板"""
    with st.expander("📦 批量操作面板", expanded=True):
        service_names = [s.get('name') for s in services]

        selected_services = st.multiselect(
            "选择要操作的服务",
            service_names,
            key="batch_selected_services"
        )

        if selected_services:
            col1, col2, col3, col4 = st.columns(4)

            with col1:
                if st.button("🔄 批量重启", key="batch_restart_btn"):
                    batch_restart_services(selected_services)

            with col2:
                if st.button("🔍 批量检查", key="batch_check_btn"):
                    batch_check_services(selected_services)

            with col3:
                if st.button("📊 批量状态", key="batch_status_btn"):
                    batch_get_status(selected_services)

            with col4:
                if st.button("🗑️ 批量删除", key="batch_delete_btn", type="secondary"):
                    if st.session_state.get('confirm_batch_delete'):
                        batch_delete_services(selected_services)
                        st.session_state.confirm_batch_delete = False
                    else:
                        st.session_state.confirm_batch_delete = True
                        st.warning("⚠️ 再次点击确认删除")
        else:
            st.info("请选择要操作的服务")

def show_mcp_services_selection():
    """显示MCP服务选择界面"""
    services_to_register = st.session_state.get('mcp_services_to_register', [])

    if not services_to_register:
        return

    st.markdown("---")
    st.markdown("#### 📋 选择要注册的服务")
    st.info(f"从MCP配置文件中找到 {len(services_to_register)} 个可注册的服务")

    # 获取当前已注册的服务名称
    api_client = st.session_state.api_client
    # 对应API: GET /for_store/list_services
    # 实际调用: store.for_store().list_services()
    current_services_response = api_client.list_services()
    current_service_names = []
    if current_services_response and current_services_response.get('data'):
        current_service_names = [s.get('name') for s in current_services_response['data']]

    # 显示服务列表供用户选择
    selected_services = []

    for i, service in enumerate(services_to_register):
        service_name = service.get('name')
        service_url = service.get('url')
        service_desc = service.get('description', '无描述')
        service_transport = service.get('transport', 'auto')

        # 检查是否已存在
        already_exists = service_name in current_service_names

        with st.container():
            col1, col2, col3 = st.columns([1, 3, 1])

            with col1:
                if already_exists:
                    st.warning("已存在")
                    selected = False
                else:
                    selected = st.checkbox(
                        "选择",
                        key=f"select_mcp_service_{i}",
                        value=True,
                        help=f"选择注册服务: {service_name}"
                    )

            with col2:
                st.markdown(f"**{service_name}**")
                st.caption(f"URL: {service_url}")
                st.caption(f"传输: {service_transport} | 描述: {service_desc}")

            with col3:
                if already_exists:
                    st.markdown("🔄 已注册")
                else:
                    st.markdown("🆕 新服务")

            if selected and not already_exists:
                selected_services.append(service)

            st.markdown("---")

    # 操作按钮
    col1, col2, col3 = st.columns([1, 1, 2])

    with col1:
        if selected_services and st.button("🚀 注册选中服务", key="register_selected_mcp_services", type="primary"):
            register_mcp_services(selected_services)

    with col2:
        if st.button("❌ 取消", key="cancel_mcp_registration"):
            if 'mcp_services_to_register' in st.session_state:
                del st.session_state['mcp_services_to_register']
            st.rerun()

    with col3:
        st.info(f"已选择 {len(selected_services)} 个服务进行注册")

def register_mcp_services(services_to_register: List[Dict]):
    """注册选中的MCP服务"""
    try:
        api_client = st.session_state.api_client

        with st.spinner(f"注册 {len(services_to_register)} 个服务..."):
            # 对应API: POST /for_store/batch_add_services
            # 实际调用: store.for_store().add_service() (批量执行)
            response = api_client.batch_add_services(services_to_register)

            if not response:
                show_error_message("API响应为空，请检查服务器连接")
                return

            if response.get('success'):
                summary = response.get('data', {}).get('summary', {})
                success_count = summary.get('succeeded', 0)  # 修正字段名
                total_count = summary.get('total', 0)
                failed_count = summary.get('failed', 0)

                show_success_message(f"MCP服务注册完成: {success_count}/{total_count} 个服务注册成功")

                # 显示详细结果
                results = response.get('data', {}).get('results', [])
                if results:
                    with st.expander("📊 详细注册结果", expanded=True):
                        for result in results:
                            # 修正数据结构解析
                            service_info = result.get('service', {})
                            service_name = service_info.get('name', 'Unknown')
                            success = result.get('success', False)

                            if success:
                                st.success(f"✅ {service_name}: 注册成功")
                            else:
                                error = result.get('message', '未知错误')
                                st.error(f"❌ {service_name}: {error}")

                # 如果有失败的服务，显示警告
                if failed_count > 0:
                    st.warning(f"⚠️ {failed_count} 个服务注册失败，请查看详细结果")

                # 清理状态并刷新页面
                if 'mcp_services_to_register' in st.session_state:
                    del st.session_state['mcp_services_to_register']
                st.rerun()
            else:
                error_msg = response.get('message', '未知错误')
                show_error_message(f"MCP服务注册失败: {error_msg}")

    except Exception as e:
        show_error_message(f"注册过程中发生异常: {str(e)}")
        import traceback
        st.error(f"详细错误: {traceback.format_exc()}")

# ==================== 新增辅助函数 ====================

def read_and_register_from_mcpconfig():
    """读取MCP配置文件并注册服务"""
    api_client = st.session_state.api_client

    with st.spinner("读取MCP配置文件..."):
        # 对应API: GET /for_store/show_mcpconfig
        # 实际调用: store.for_store().show_mcpconfig()
        response = api_client.show_mcpconfig()

        if not response or not response.get('success'):
            show_error_message("无法读取MCP配置文件")
            return

        # API直接返回配置数据，不需要解析JSON字符串
        mcp_config = response.get('data', {})

        try:

            # 提取服务配置
            mcpServers = mcp_config.get('mcpServers', {})

            if not mcpServers:
                show_warning_message("MCP配置文件中未找到服务配置")
                return

            # 显示可注册的服务
            st.success(f"找到 {len(mcpServers)} 个服务配置")

            services_to_register = []
            for server_name, server_config in mcpServers.items():
                if isinstance(server_config, dict):
                    # 检查是否是简化格式（直接包含url字段）
                    if 'url' in server_config:
                        # 简化格式：直接包含url、transport等字段
                        service_config = {
                            "name": server_name,
                            "url": server_config['url'],
                            "description": server_config.get('description', f"从MCP配置导入: {server_name}")
                        }

                        # 添加可选字段
                        if 'transport' in server_config:
                            service_config["transport"] = server_config['transport']

                        if 'timeout' in server_config:
                            service_config["timeout"] = server_config['timeout']

                        if 'headers' in server_config:
                            service_config["headers"] = server_config['headers']

                        if 'env' in server_config:
                            service_config["env"] = server_config['env']

                        services_to_register.append(service_config)

                    else:
                        # 标准格式：包含command、args等字段
                        command = server_config.get('command')
                        args = server_config.get('args', [])
                        env = server_config.get('env', {})

                        # 尝试从args中提取URL
                        url = None
                        if args:
                            for arg in args:
                                if isinstance(arg, str) and (arg.startswith('http') or '/mcp' in arg):
                                    url = arg
                                    break

                        if url:
                            service_config = {
                                "name": server_name,
                                "url": url,
                                "description": f"从MCP配置导入: {command}"
                            }

                            if env:
                                service_config["env"] = env

                            services_to_register.append(service_config)

            if services_to_register:
                # 显示找到的服务并让用户选择
                st.session_state.mcp_services_to_register = services_to_register
                st.rerun()
            else:
                show_warning_message("未找到可注册的服务URL")

        except Exception as e:
            show_error_message(f"处理MCP配置时出错: {str(e)}")

def add_service_from_form(name: str, url: str, transport: str, description: str,
                         keep_alive: bool, timeout: int, headers_text: str, env_text: str):
    """从表单添加服务"""
    # 验证输入
    if not validate_service_name(name):
        show_error_message("服务名称无效：只能包含字母、数字、下划线和连字符")
        return

    if not validate_url(url):
        show_error_message("URL格式无效")
        return

    # 构建配置
    config = {
        "name": name,
        "url": url
    }

    if transport != "auto":
        config["transport"] = transport

    if description.strip():
        config["description"] = description.strip()

    if keep_alive:
        config["keep_alive"] = True

    if timeout != 30:
        config["timeout"] = timeout

    # 解析headers
    if headers_text.strip():
        try:
            config["headers"] = json.loads(headers_text)
        except json.JSONDecodeError:
            show_error_message("请求头JSON格式错误")
            return

    # 解析环境变量
    if env_text.strip():
        try:
            config["env"] = json.loads(env_text)
        except json.JSONDecodeError:
            show_error_message("环境变量JSON格式错误")
            return

    # 添加服务
    api_client = st.session_state.api_client

    with st.spinner(f"添加服务 {name}..."):
        # 对应API: POST /for_store/add_service
        # 实际调用: store.for_store().add_service(config)
        response = api_client.add_service(config)

        if response and response.get('success'):
            show_success_message(f"服务 {name} 添加成功")
            st.rerun()
        else:
            error_msg = response.get('message', '未知错误') if response else '请求失败'
            show_error_message(f"服务 {name} 添加失败: {error_msg}")

def add_service_from_json(json_config: str):
    """从JSON配置添加单个服务"""
    try:
        config = json.loads(json_config)

        if not isinstance(config, dict):
            show_error_message("JSON配置必须是对象格式")
            return

        # 验证必填字段
        if not config.get('name'):
            show_error_message("缺少必填字段: name")
            return

        if not config.get('url'):
            show_error_message("缺少必填字段: url")
            return

        # 验证字段
        if not validate_service_name(config['name']):
            show_error_message("服务名称无效")
            return

        if not validate_url(config['url']):
            show_error_message("URL格式无效")
            return

        # 添加服务
        api_client = st.session_state.api_client

        with st.spinner(f"添加服务 {config['name']}..."):
            # 对应API: POST /for_store/add_service
            # 实际调用: store.for_store().add_service(config)
            response = api_client.add_service(config)

            if response and response.get('success'):
                show_success_message(f"服务 {config['name']} 添加成功")
                st.rerun()
            else:
                error_msg = response.get('message', '未知错误') if response else '请求失败'
                show_error_message(f"服务 {config['name']} 添加失败: {error_msg}")

    except json.JSONDecodeError as e:
        show_error_message(f"JSON格式错误: {str(e)}")

def validate_batch_config(json_config: str):
    """验证批量配置"""
    try:
        services = json.loads(json_config)

        if not isinstance(services, list):
            show_error_message("批量配置必须是数组格式")
            return

        errors = []
        warnings = []

        for i, service in enumerate(services):
            if not isinstance(service, dict):
                errors.append(f"第 {i+1} 个服务配置不是对象格式")
                continue

            # 检查必填字段
            if not service.get('name'):
                errors.append(f"第 {i+1} 个服务缺少 name 字段")
            elif not validate_service_name(service['name']):
                errors.append(f"第 {i+1} 个服务名称格式无效: {service['name']}")

            if not service.get('url'):
                errors.append(f"第 {i+1} 个服务缺少 url 字段")
            elif not validate_url(service['url']):
                errors.append(f"第 {i+1} 个服务URL格式无效: {service['url']}")

            # 检查可选字段
            if service.get('transport') and service['transport'] not in ['auto', 'sse', 'streamable-http']:
                warnings.append(f"第 {i+1} 个服务传输类型可能无效: {service['transport']}")

        if errors:
            st.error("❌ 配置验证失败:")
            for error in errors:
                st.write(f"• {error}")
        else:
            st.success("✅ 配置验证通过!")
            st.write(f"• 共 {len(services)} 个服务配置")
            st.write(f"• 所有必填字段完整")

            if warnings:
                st.warning("⚠️ 注意事项:")
                for warning in warnings:
                    st.write(f"• {warning}")

    except json.JSONDecodeError as e:
        show_error_message(f"JSON格式错误: {str(e)}")

def show_service_edit_form(service_name: str, service_info: Dict):
    """显示服务编辑表单"""
    st.markdown(f"#### ✏️ 编辑服务: {service_name}")
    st.info("注意: 服务名称不可修改，其他配置项可以修改")

    with st.form(f"edit_service_form_{service_name}"):
        col1, col2 = st.columns(2)

        with col1:
            # 服务名称（只读）
            st.text_input(
                "服务名称",
                value=service_name,
                disabled=True,
                help="服务名称不可修改"
            )

            # URL
            new_url = st.text_input(
                "服务URL *",
                value=service_info.get('url', ''),
                help="MCP服务的完整URL地址"
            )

            # 传输类型
            current_transport = service_info.get('transport', 'auto')
            new_transport = st.selectbox(
                "传输类型",
                ["auto", "sse", "streamable-http"],
                index=["auto", "sse", "streamable-http"].index(current_transport) if current_transport in ["auto", "sse", "streamable-http"] else 0
            )

        with col2:
            # 描述
            new_description = st.text_area(
                "服务描述",
                value=service_info.get('description', ''),
                help="服务的功能描述"
            )

            # 保持连接
            new_keep_alive = st.checkbox(
                "保持连接",
                value=service_info.get('keep_alive', False),
                help="是否保持长连接"
            )

            # 超时时间
            new_timeout = st.number_input(
                "超时时间(秒)",
                min_value=1,
                max_value=300,
                value=service_info.get('timeout', 30),
                help="请求超时时间"
            )

        # 高级选项
        with st.expander("🔧 高级选项"):
            # 请求头
            current_headers = service_info.get('headers', {})
            new_headers_text = st.text_area(
                "请求头 (JSON格式)",
                value=json.dumps(current_headers, indent=2, ensure_ascii=False) if current_headers else '',
                help="自定义HTTP请求头"
            )

            # 环境变量
            current_env = service_info.get('env', {})
            new_env_text = st.text_area(
                "环境变量 (JSON格式)",
                value=json.dumps(current_env, indent=2, ensure_ascii=False) if current_env else '',
                help="服务运行时的环境变量"
            )

        # 提交按钮
        col1, col2, col3 = st.columns([1, 1, 2])

        with col1:
            submitted = st.form_submit_button("💾 保存修改", type="primary")

        with col2:
            cancelled = st.form_submit_button("❌ 取消")

        if cancelled:
            if 'edit_service_detail' in st.session_state:
                del st.session_state['edit_service_detail']
            st.rerun()

        if submitted:
            update_service_config(service_name, new_url, new_transport, new_description,
                                new_keep_alive, new_timeout, new_headers_text, new_env_text)

def update_service_config(service_name: str, url: str, transport: str, description: str,
                         keep_alive: bool, timeout: int, headers_text: str, env_text: str):
    """更新服务配置"""
    # 验证输入
    if not validate_url(url):
        show_error_message("URL格式无效")
        return

    # 构建新配置
    config = {
        "name": service_name,  # 名称不变
        "url": url
    }

    if transport != "auto":
        config["transport"] = transport

    if description.strip():
        config["description"] = description.strip()

    if keep_alive:
        config["keep_alive"] = True

    if timeout != 30:
        config["timeout"] = timeout

    # 解析headers
    if headers_text.strip():
        try:
            config["headers"] = json.loads(headers_text)
        except json.JSONDecodeError:
            show_error_message("请求头JSON格式错误")
            return

    # 解析环境变量
    if env_text.strip():
        try:
            config["env"] = json.loads(env_text)
        except json.JSONDecodeError:
            show_error_message("环境变量JSON格式错误")
            return

    # 更新服务
    api_client = st.session_state.api_client

    with st.spinner(f"更新服务 {service_name}..."):
        # 对应API: POST /for_store/update_service
        # 实际调用: store.for_store().update_service(config)
        response = api_client.update_service(service_name, config)

        if response and response.get('success'):
            show_success_message(f"服务 {service_name} 更新成功")
            # 清理编辑状态
            if 'edit_service_detail' in st.session_state:
                del st.session_state['edit_service_detail']
            st.rerun()
        else:
            error_msg = response.get('message', '未知错误') if response else '请求失败'
            show_error_message(f"服务 {service_name} 更新失败: {error_msg}")

def batch_restart_services(service_names: List[str]):
    """批量重启服务"""
    api_client = st.session_state.api_client

    with st.spinner(f"批量重启 {len(service_names)} 个服务..."):
        # 对应API: POST /for_store/batch_restart_services
        # 实际调用: store.for_store().restart_service() (批量执行)
        response = api_client.batch_restart_services(service_names)

        if response and response.get('success'):
            summary = response.get('data', {}).get('summary', {})
            success_count = summary.get('succeeded', 0)  # 修正字段名
            total_count = summary.get('total', 0)
            failed_count = summary.get('failed', 0)
            show_success_message(f"批量重启完成: {success_count}/{total_count} 个服务重启成功")

            if failed_count > 0:
                st.warning(f"⚠️ {failed_count} 个服务重启失败")
            st.rerun()
        else:
            error_msg = response.get('message', '未知错误') if response else '请求失败'
            show_error_message(f"批量重启失败: {error_msg}")

def batch_check_services(service_names: List[str]):
    """批量检查服务"""
    api_client = st.session_state.api_client

    with st.spinner(f"批量检查 {len(service_names)} 个服务..."):
        # 对应API: GET /for_store/check_services
        # 实际调用: store.for_store().check_services()
        response = api_client.check_services()

        if response:
            show_success_message("批量健康检查完成")
            st.rerun()
        else:
            show_error_message("批量健康检查失败")

def batch_get_status(service_names: List[str]):
    """批量获取服务状态"""
    api_client = st.session_state.api_client

    with st.spinner(f"获取 {len(service_names)} 个服务状态..."):
        results = []

        for service_name in service_names:
            try:
                response = api_client.get_service_status(service_name)
                if response:
                    results.append({
                        'name': service_name,
                        'status': response.get('data', {}),
                        'success': True
                    })
                else:
                    results.append({
                        'name': service_name,
                        'error': '获取状态失败',
                        'success': False
                    })
            except Exception as e:
                results.append({
                    'name': service_name,
                    'error': str(e),
                    'success': False
                })

        # 显示结果
        success_count = sum(1 for r in results if r['success'])
        show_success_message(f"状态查询完成: {success_count}/{len(service_names)} 个服务")

        # 显示详细结果
        for result in results:
            if result['success']:
                st.success(f"✅ {result['name']}: 状态正常")
            else:
                st.error(f"❌ {result['name']}: {result.get('error', '未知错误')}")

def batch_delete_services(service_names: List[str]):
    """批量删除服务"""
    api_client = st.session_state.api_client

    with st.spinner(f"批量删除 {len(service_names)} 个服务..."):
        # 对应API: POST /for_store/batch_delete_services
        # 实际调用: store.for_store().delete_service() (批量执行)
        response = api_client.batch_delete_services(service_names)

        if response and response.get('success'):
            summary = response.get('data', {}).get('summary', {})
            success_count = summary.get('succeeded', 0)  # 修正字段名
            total_count = summary.get('total', 0)
            failed_count = summary.get('failed', 0)
            show_success_message(f"批量删除完成: {success_count}/{total_count} 个服务删除成功")

            if failed_count > 0:
                st.warning(f"⚠️ {failed_count} 个服务删除失败")
            st.rerun()
        else:
            error_msg = response.get('message', '未知错误') if response else '请求失败'
            show_error_message(f"批量删除失败: {error_msg}")

def get_service_status(service_name: str):
    """获取服务详细状态"""
    api_client = st.session_state.api_client

    with st.spinner(f"获取服务 {service_name} 状态..."):
        response = api_client.get_service_status(service_name)

        if response and response.get('success'):
            status_data = response.get('data', {})

            # 显示状态信息
            st.success("✅ 服务状态获取成功")

            with st.expander("📊 详细状态信息", expanded=True):
                col1, col2 = st.columns(2)

                with col1:
                    st.markdown("**连接信息**:")
                    health = status_data.get('health', {})
                    st.write(f"• 健康状态: {health.get('status', 'unknown')}")
                    st.write(f"• 响应时间: {health.get('response_time', 'N/A')}")
                    st.write(f"• 最后检查: {health.get('last_check', 'N/A')}")

                with col2:
                    st.markdown("**服务信息**:")
                    service_info = status_data.get('service', {})
                    st.write(f"• 服务名称: {service_info.get('name', 'N/A')}")
                    st.write(f"• 服务URL: {service_info.get('url', 'N/A')}")
                    st.write(f"• 传输类型: {service_info.get('transport', 'N/A')}")

                # 完整状态数据
                st.markdown("**完整状态数据**:")
                st.code(format_json(status_data), language='json')
        else:
            show_error_message(f"获取服务 {service_name} 状态失败")

# ==================== 原有辅助函数 ====================

def check_all_services_health():
    """检查所有服务健康状态"""
    api_client = st.session_state.api_client
    
    with st.spinner("检查服务健康状态..."):
        response = api_client.check_services()
        
        if response:
            show_success_message("健康检查完成")
            st.rerun()
        else:
            show_error_message("健康检查失败")

def restart_service(service_name: str):
    """重启服务"""
    api_client = st.session_state.api_client
    
    with st.spinner(f"重启服务 {service_name}..."):
        response = api_client.restart_service(service_name)
        
        if response and response.get('success'):
            show_success_message(f"服务 {service_name} 重启成功")
            st.rerun()
        else:
            show_error_message(f"服务 {service_name} 重启失败")

def delete_service(service_name: str):
    """删除服务"""
    # 确认删除
    if not st.session_state.get(f'confirm_delete_{service_name}'):
        st.session_state[f'confirm_delete_{service_name}'] = True
        show_warning_message(f"确认删除服务 {service_name}？再次点击删除按钮确认。")
        return
    
    api_client = st.session_state.api_client
    
    with st.spinner(f"删除服务 {service_name}..."):
        response = api_client.delete_service(service_name)
        
        if response and response.get('success'):
            show_success_message(f"服务 {service_name} 删除成功")
            # 清理确认状态
            if f'confirm_delete_{service_name}' in st.session_state:
                del st.session_state[f'confirm_delete_{service_name}']
            st.rerun()
        else:
            show_error_message(f"服务 {service_name} 删除失败")

def add_preset_service(preset: Dict):
    """添加预设服务"""
    api_client = st.session_state.api_client
    
    with st.spinner(f"添加服务 {preset['name']}..."):
        response = api_client.add_service({
            "name": preset['name'],
            "url": preset['url']
        })
        
        if response and response.get('success'):
            show_success_message(f"服务 {preset['name']} 添加成功")
            st.rerun()
        else:
            show_error_message(f"服务 {preset['name']} 添加失败")

def add_custom_service(name: str, url: str, transport: str, keep_alive: bool, headers_text: str, env_text: str):
    """添加自定义服务"""
    # 验证输入
    if not validate_service_name(name):
        show_error_message("服务名称无效")
        return
    
    if not validate_url(url):
        show_error_message("URL格式无效")
        return
    
    # 构建配置
    config = {
        "name": name,
        "url": url
    }
    
    if transport != "auto":
        config["transport"] = transport
    
    if keep_alive:
        config["keep_alive"] = True
    
    # 解析headers
    if headers_text.strip():
        try:
            config["headers"] = json.loads(headers_text)
        except json.JSONDecodeError:
            show_error_message("请求头JSON格式错误")
            return
    
    # 解析环境变量
    if env_text.strip():
        try:
            config["env"] = json.loads(env_text)
        except json.JSONDecodeError:
            show_error_message("环境变量JSON格式错误")
            return
    
    # 添加服务
    api_client = st.session_state.api_client
    
    with st.spinner(f"添加服务 {name}..."):
        response = api_client.add_service(config)
        
        if response and response.get('success'):
            show_success_message(f"服务 {name} 添加成功")
            st.rerun()
        else:
            show_error_message(f"服务 {name} 添加失败")

def batch_add_from_json(json_config: str):
    """从JSON配置批量添加服务"""
    try:
        services = json.loads(json_config)
        
        if not isinstance(services, list):
            show_error_message("JSON配置必须是数组格式")
            return
        
        api_client = st.session_state.api_client
        
        with st.spinner("批量添加服务..."):
            # 对应API: POST /for_store/batch_add_services
            # 实际调用: store.for_store().add_service() (批量执行)
            response = api_client.batch_add_services(services)
            
            if response and response.get('success'):
                summary = response.get('data', {}).get('summary', {})
                success_count = summary.get('succeeded', 0)
                total_count = summary.get('total', 0)
                failed_count = summary.get('failed', 0)

                show_success_message(f"批量添加完成: {success_count}/{total_count} 个服务添加成功")

                if failed_count > 0:
                    st.warning(f"⚠️ {failed_count} 个服务添加失败")

                    # 显示详细结果
                    results = response.get('data', {}).get('results', [])
                    if results:
                        with st.expander("📊 详细添加结果"):
                            for result in results:
                                service_info = result.get('service', {})
                                service_name = service_info.get('name', 'Unknown')
                                success = result.get('success', False)

                                if success:
                                    st.success(f"✅ {service_name}: 添加成功")
                                else:
                                    error = result.get('message', '未知错误')
                                    st.error(f"❌ {service_name}: {error}")

                st.rerun()
            else:
                error_msg = response.get('message', '未知错误') if response else '请求失败'
                show_error_message(f"批量添加失败: {error_msg}")
    
    except json.JSONDecodeError:
        show_error_message("JSON格式错误")

def batch_add_from_csv(uploaded_file):
    """从CSV文件批量添加服务"""
    try:
        # 简单的CSV解析，不依赖pandas
        import csv
        import io

        # 读取文件内容
        content = uploaded_file.read().decode('utf-8')
        csv_reader = csv.DictReader(io.StringIO(content))

        services = []
        for row in csv_reader:
            service = {
                "name": row.get('name', ''),
                "url": row.get('url', '')
            }

            if 'transport' in row and row['transport']:
                service['transport'] = row['transport']

            services.append(service)

        api_client = st.session_state.api_client

        with st.spinner("批量添加服务..."):
            response = api_client.batch_add_services(services)

            if response and response.get('success'):
                show_success_message(f"成功批量添加 {len(services)} 个服务")
                st.rerun()
            else:
                show_error_message("批量添加失败")

    except Exception as e:
        show_error_message(f"CSV处理失败: {e}")

def batch_restart_services(service_names: List[str]):
    """批量重启服务"""
    api_client = st.session_state.api_client
    
    success_count = 0
    
    with st.spinner("批量重启服务..."):
        for service_name in service_names:
            response = api_client.restart_service(service_name)
            if response and response.get('success'):
                success_count += 1
    
    show_success_message(f"成功重启 {success_count}/{len(service_names)} 个服务")
    st.rerun()

def batch_check_services(service_names: List[str]):
    """批量检查服务"""
    api_client = st.session_state.api_client

    with st.spinner("批量检查服务..."):
        response = api_client.check_services()

        if response:
            show_success_message("批量检查完成")
            st.rerun()
        else:
            show_error_message("批量检查失败")

def get_service_status(service_name: str):
    """获取服务详细状态"""
    api_client = st.session_state.api_client

    with st.spinner(f"获取服务 {service_name} 状态..."):
        # 使用新的服务状态API（如果可用）
        try:
            response = api_client._request('POST', '/for_store/get_service_status', json={"name": service_name})
            if response and response.get('success'):
                status_data = response.get('data', {})

                with st.expander(f"📊 {service_name} 详细状态", expanded=True):
                    col1, col2 = st.columns(2)

                    with col1:
                        st.markdown("**服务信息**:")
                        service_info = status_data.get('service', {})
                        if isinstance(service_info, dict):
                            for key, value in service_info.items():
                                if key != 'tools':  # 工具信息单独显示
                                    st.write(f"- {key}: {value}")

                    with col2:
                        st.markdown("**健康状态**:")
                        health_info = status_data.get('health', {})
                        if health_info:
                            st.write(f"- 状态: {health_info.get('status', 'unknown')}")
                            st.write(f"- 最后检查: {status_data.get('last_check', 'N/A')}")

                        tools_info = status_data.get('tools', {})
                        st.metric("工具数量", tools_info.get('count', 0))

                show_success_message(f"服务 {service_name} 状态获取成功")
            else:
                show_error_message(f"获取服务 {service_name} 状态失败")
        except Exception as e:
            show_error_message(f"获取服务状态时发生错误: {e}")

def show_service_edit_form(service_name: str, service_info: Dict):
    """显示服务编辑表单"""
    st.markdown("#### ✏️ 编辑服务配置")

    with st.form(f"edit_service_form_{service_name}"):
        col1, col2 = st.columns(2)

        with col1:
            new_url = st.text_input(
                "服务URL",
                value=service_info.get('url', ''),
                help="更新服务的URL地址"
            )

            new_transport = st.selectbox(
                "传输类型",
                ["auto", "sse", "streamable-http"],
                index=["auto", "sse", "streamable-http"].index(service_info.get('transport', 'auto')),
                help="选择传输协议类型"
            )

        with col2:
            new_keep_alive = st.checkbox(
                "保持连接",
                value=service_info.get('keep_alive', False),
                help="是否保持长连接"
            )

            new_timeout = st.number_input(
                "超时时间(秒)",
                min_value=1,
                max_value=300,
                value=service_info.get('timeout', 30),
                help="请求超时时间"
            )

        # 高级配置
        with st.expander("🔧 高级配置"):
            headers_text = st.text_area(
                "请求头 (JSON格式)",
                value=json.dumps(service_info.get('headers', {}), indent=2) if service_info.get('headers') else '',
                help="自定义HTTP请求头"
            )

            env_text = st.text_area(
                "环境变量 (JSON格式)",
                value=json.dumps(service_info.get('env', {}), indent=2) if service_info.get('env') else '',
                help="服务运行时的环境变量"
            )

        col1, col2 = st.columns(2)

        with col1:
            submitted = st.form_submit_button("💾 保存更改", type="primary")

        with col2:
            cancelled = st.form_submit_button("❌ 取消")

        if submitted:
            update_service_config(service_name, {
                "url": new_url,
                "transport": new_transport if new_transport != "auto" else None,
                "keep_alive": new_keep_alive,
                "timeout": new_timeout,
                "headers": json.loads(headers_text) if headers_text.strip() else {},
                "env": json.loads(env_text) if env_text.strip() else {}
            })

        if cancelled:
            if 'edit_service_detail' in st.session_state:
                del st.session_state.edit_service_detail
            st.rerun()

def update_service_config(service_name: str, config: Dict):
    """更新服务配置"""
    api_client = st.session_state.api_client

    try:
        with st.spinner(f"更新服务 {service_name} 配置..."):
            response = api_client.update_service(service_name, config)

            if response and response.get('success'):
                show_success_message(f"服务 {service_name} 配置更新成功")
                # 清除编辑状态
                if 'edit_service_detail' in st.session_state:
                    del st.session_state.edit_service_detail
                st.rerun()
            else:
                show_error_message(f"服务 {service_name} 配置更新失败")

    except json.JSONDecodeError:
        show_error_message("JSON格式错误，请检查请求头或环境变量配置")
    except Exception as e:
        show_error_message(f"更新服务配置时发生错误: {e}")

def batch_delete_services(service_names: List[str]):
    """批量删除服务"""
    api_client = st.session_state.api_client
    
    success_count = 0
    
    with st.spinner("批量删除服务..."):
        for service_name in service_names:
            response = api_client.delete_service(service_name)
            if response and response.get('success'):
                success_count += 1
    
    show_success_message(f"成功删除 {success_count}/{len(service_names)} 个服务")
    
    # 清理确认状态
    st.session_state.confirm_batch_delete = False
    st.rerun()
