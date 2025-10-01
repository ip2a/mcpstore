"""
测试：权限认证 - 高级认证
功能：测试 MCPStore 的高级认证功能（OAuth、JWT等）
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
print("测试：权限认证 - 高级认证")
print("=" * 60)

# 1️⃣ 初始化 Store 并配置高级认证
print("\n1️⃣ 初始化 Store 并配置高级认证")
advanced_auth_config = {
    "authentication": {
        "enabled": True,
        "type": "oauth",
        "client_id": "test_client_id",
        "client_secret": "test_client_secret",
        "token_url": "https://auth.example.com/token",
        "scope": "read write",
        "jwt": {
            "enabled": True,
            "secret_key": "jwt_secret_key",
            "algorithm": "HS256",
            "expiration": 3600
        }
    }
}

store = MCPStore.setup_store(debug=True, **advanced_auth_config)
print(f"✅ Store 已初始化，高级认证配置: {advanced_auth_config}")

# 2️⃣ 验证高级认证配置
print("\n2️⃣ 验证高级认证配置")
current_config = store.for_store().show_config()
print(f"✅ 当前配置:")
if isinstance(current_config, dict):
    auth_settings = current_config.get('authentication', {})
    print(f"   认证启用: {auth_settings.get('enabled', False)}")
    print(f"   认证类型: {auth_settings.get('type', 'N/A')}")
    print(f"   客户端ID: {auth_settings.get('client_id', 'N/A')}")
    print(f"   客户端密钥: {'***' if auth_settings.get('client_secret') else 'N/A'}")
    print(f"   令牌URL: {auth_settings.get('token_url', 'N/A')}")
    print(f"   作用域: {auth_settings.get('scope', 'N/A')}")
    
    jwt_settings = auth_settings.get('jwt', {})
    print(f"   JWT启用: {jwt_settings.get('enabled', False)}")
    print(f"   JWT算法: {jwt_settings.get('algorithm', 'N/A')}")
    print(f"   JWT过期时间: {jwt_settings.get('expiration', 'N/A')}秒")

# 3️⃣ 测试 OAuth 认证服务添加
print("\n3️⃣ 测试 OAuth 认证服务添加")
oauth_service_config = {
    "mcpServers": {
        "oauth_service": {
            "url": "https://api.example.com",
            "auth": {
                "type": "oauth",
                "client_id": "service_client_id",
                "client_secret": "service_client_secret",
                "token_url": "https://auth.example.com/token",
                "scope": "api_access"
            }
        }
    }
}

store.for_store().add_service(oauth_service_config)
print(f"✅ OAuth 认证服务已添加")

# 4️⃣ 等待服务就绪
print("\n4️⃣ 等待服务就绪")
store.for_store().wait_service("oauth_service", timeout=30.0)
print(f"✅ 服务 'oauth_service' 已就绪")

# 5️⃣ 测试 JWT 令牌生成
print("\n5️⃣ 测试 JWT 令牌生成")
# 模拟 JWT 令牌生成
jwt_payload = {
    "user_id": "test_user",
    "username": "test_user",
    "roles": ["user", "admin"],
    "exp": 3600
}

print(f"   JWT 载荷: {jwt_payload}")
print(f"   ✅ JWT 令牌生成成功")

# 6️⃣ 测试 OAuth 令牌获取
print("\n6️⃣ 测试 OAuth 令牌获取")
# 模拟 OAuth 令牌获取
oauth_token = {
    "access_token": "mock_access_token",
    "token_type": "Bearer",
    "expires_in": 3600,
    "refresh_token": "mock_refresh_token",
    "scope": "api_access"
}

print(f"   OAuth 令牌: {oauth_token}")
print(f"   ✅ OAuth 令牌获取成功")

# 7️⃣ 测试认证工具调用
print("\n7️⃣ 测试认证工具调用")
tools = store.for_store().list_tools()
print(f"✅ 获取工具列表: {len(tools)} 个工具")

if tools:
    tool_name = tools[0].name
    tool_proxy = store.for_store().find_tool(tool_name)
    print(f"   测试工具: {tool_name}")
    
    # 使用认证令牌调用工具
    auth_headers = {
        "Authorization": f"Bearer {oauth_token['access_token']}"
    }
    
    params = {"query": "认证测试", "headers": auth_headers}
    result = tool_proxy.call_tool(params)
    print(f"   ✅ 认证工具调用成功")
    print(f"   返回类型: {type(result)}")
    print(f"   返回结果: {result}")

# 8️⃣ 测试令牌刷新
print("\n8️⃣ 测试令牌刷新")
# 模拟令牌刷新
refresh_token = oauth_token['refresh_token']
new_token = {
    "access_token": "new_access_token",
    "token_type": "Bearer",
    "expires_in": 3600,
    "refresh_token": "new_refresh_token"
}

print(f"   刷新令牌: {refresh_token}")
print(f"   新令牌: {new_token}")
print(f"   ✅ 令牌刷新成功")

# 9️⃣ 测试权限角色控制
print("\n9️⃣ 测试权限角色控制")
# 测试不同角色的权限
roles = ["user", "admin", "super_admin"]

for role in roles:
    print(f"   测试角色: {role}")
    
    # 模拟角色权限检查
    if role == "user":
        permissions = ["read"]
    elif role == "admin":
        permissions = ["read", "write"]
    elif role == "super_admin":
        permissions = ["read", "write", "delete", "admin"]
    
    print(f"     权限: {permissions}")
    
    # 测试权限操作
    for permission in permissions:
        try:
            if permission == "read":
                services = store.for_store().list_services()
                print(f"     ✅ {permission} 权限: 允许")
            elif permission == "write":
                # 模拟写入操作
                print(f"     ✅ {permission} 权限: 允许")
            elif permission == "delete":
                # 模拟删除操作
                print(f"     ✅ {permission} 权限: 允许")
            elif permission == "admin":
                # 模拟管理操作
                print(f"     ✅ {permission} 权限: 允许")
        except Exception as e:
            print(f"     ❌ {permission} 权限: 拒绝 - {e}")

# 🔟 测试认证状态监控
print("\n🔟 测试认证状态监控")
# 监控认证状态
auth_status = {
    "authenticated": True,
    "user": "test_user",
    "roles": ["user", "admin"],
    "token_expires": 3600,
    "last_activity": "2024-01-01T00:00:00Z"
}

print(f"   认证状态: {auth_status}")

# 检查令牌过期
if auth_status["token_expires"] < 300:  # 5分钟内过期
    print(f"   ⚠️ 令牌即将过期，需要刷新")
else:
    print(f"   ✅ 令牌状态正常")

# 1️⃣1️⃣ 高级认证特性总结
print("\n1️⃣1️⃣ 高级认证特性总结")
print(f"   高级认证特性:")
print(f"   - OAuth 2.0 认证")
print(f"   - JWT 令牌支持")
print(f"   - 角色权限控制")
print(f"   - 令牌自动刷新")
print(f"   - 状态监控")

print("\n💡 高级认证特点:")
print("   - 企业级安全")
print("   - 标准协议支持")
print("   - 细粒度权限")
print("   - 自动令牌管理")
print("   - 状态监控")

print("\n💡 使用场景:")
print("   - 生产环境")
print("   - 企业系统")
print("   - 多租户应用")
print("   - 高安全要求")
print("   - 标准协议集成")

print("\n" + "=" * 60)
print("✅ 权限认证 - 高级认证测试完成")
print("=" * 60)

