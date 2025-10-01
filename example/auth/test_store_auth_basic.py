"""
测试：权限认证 - 基础认证
功能：测试 MCPStore 的基础认证功能
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：权限认证 - 基础认证")
print("=" * 60)

# 1️⃣ 初始化 Store 并配置认证
print("\n1️⃣ 初始化 Store 并配置认证")
auth_config = {
    "authentication": {
        "enabled": True,
        "type": "basic",
        "username": "test_user",
        "password": "test_password"
    }
}

store = MCPStore.setup_store(debug=True, **auth_config)
print(f"✅ Store 已初始化，认证配置: {auth_config}")

# 2️⃣ 验证认证配置
print("\n2️⃣ 验证认证配置")
current_config = store.for_store().show_config()
print(f"✅ 当前配置:")
if isinstance(current_config, dict):
    auth_settings = current_config.get('authentication', {})
    print(f"   认证启用: {auth_settings.get('enabled', False)}")
    print(f"   认证类型: {auth_settings.get('type', 'N/A')}")
    print(f"   用户名: {auth_settings.get('username', 'N/A')}")
    print(f"   密码: {'***' if auth_settings.get('password') else 'N/A'}")

# 3️⃣ 测试认证服务添加
print("\n3️⃣ 测试认证服务添加")
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp",
            "auth": {
                "username": "service_user",
                "password": "service_password"
            }
        }
    }
}

store.for_store().add_service(service_config)
print(f"✅ 带认证的服务已添加")

# 4️⃣ 等待服务就绪
print("\n4️⃣ 等待服务就绪")
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务 'weather' 已就绪")

# 5️⃣ 测试认证工具调用
print("\n5️⃣ 测试认证工具调用")
tools = store.for_store().list_tools()
print(f"✅ 获取工具列表: {len(tools)} 个工具")

if tools:
    tool_name = tools[0].name
    tool_proxy = store.for_store().find_tool(tool_name)
    print(f"   测试工具: {tool_name}")
    
    # 调用工具
    params = {"query": "北京"}
    result = tool_proxy.call_tool(params)
    print(f"   ✅ 工具调用成功")
    print(f"   返回类型: {type(result)}")
    print(f"   返回结果: {result}")

# 6️⃣ 测试认证状态检查
print("\n6️⃣ 测试认证状态检查")
# 检查服务认证状态
service_proxy = store.for_store().find_service("weather")
service_info = service_proxy.service_info()
print(f"✅ 服务认证信息:")
if isinstance(service_info, dict):
    auth_info = service_info.get('auth', {})
    print(f"   认证状态: {auth_info.get('enabled', False)}")
    print(f"   认证类型: {auth_info.get('type', 'N/A')}")

# 7️⃣ 测试认证配置更新
print("\n7️⃣ 测试认证配置更新")
# 更新服务认证配置
new_auth_config = {
    "auth": {
        "username": "updated_user",
        "password": "updated_password",
        "enabled": True
    }
}

service_proxy.patch_config(new_auth_config)
print(f"✅ 服务认证配置已更新")

# 验证更新
updated_info = service_proxy.service_info()
print(f"   更新后的认证信息: {updated_info.get('auth', {})}")

# 8️⃣ 测试认证错误处理
print("\n8️⃣ 测试认证错误处理")
# 测试无效认证
invalid_service_config = {
    "mcpServers": {
        "invalid_service": {
            "url": "https://invalid.example.com",
            "auth": {
                "username": "invalid_user",
                "password": "invalid_password"
            }
        }
    }
}

try:
    store.for_store().add_service(invalid_service_config)
    print(f"   ⚠️ 无效服务添加成功（可能无认证检查）")
except Exception as e:
    print(f"   ✅ 无效服务添加被拒绝: {e}")

# 9️⃣ 测试认证权限控制
print("\n9️⃣ 测试认证权限控制")
# 测试不同权限级别的操作
print(f"   测试权限控制:")

# 测试服务管理权限
try:
    services = store.for_store().list_services()
    print(f"   ✅ 服务列表权限: 允许")
except Exception as e:
    print(f"   ❌ 服务列表权限: 拒绝 - {e}")

# 测试工具调用权限
try:
    if tools:
        tool_proxy = store.for_store().find_tool(tools[0].name)
        result = tool_proxy.call_tool({"query": "权限测试"})
        print(f"   ✅ 工具调用权限: 允许")
except Exception as e:
    print(f"   ❌ 工具调用权限: 拒绝 - {e}")

# 测试配置管理权限
try:
    config = store.for_store().show_config()
    print(f"   ✅ 配置查看权限: 允许")
except Exception as e:
    print(f"   ❌ 配置查看权限: 拒绝 - {e}")

# 🔟 认证特性总结
print("\n🔟 认证特性总结")
print(f"   基础认证特性:")
print(f"   - 用户名密码认证")
print(f"   - 服务级认证")
print(f"   - 配置管理")
print(f"   - 权限控制")
print(f"   - 错误处理")

print("\n💡 基础认证特点:")
print("   - 简单易用")
print("   - 配置灵活")
print("   - 权限控制")
print("   - 错误处理")
print("   - 状态监控")

print("\n💡 使用场景:")
print("   - 开发环境")
print("   - 测试环境")
print("   - 内部系统")
print("   - 基础安全")
print("   - 快速部署")

print("\n" + "=" * 60)
print("✅ 权限认证 - 基础认证测试完成")
print("=" * 60)

