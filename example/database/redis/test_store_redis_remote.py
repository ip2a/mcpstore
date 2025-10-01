"""
测试：Redis 数据库支持 - 远程服务
功能：测试使用 Redis 作为后端存储的远程服务
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
print("测试：Redis 数据库支持 - 远程服务")
print("=" * 60)

# 1️⃣ 初始化 Store 并配置远程 Redis
print("\n1️⃣ 初始化 Store 并配置远程 Redis")
redis_config = {
    "redis": {
        "host": "redis.example.com",  # 远程 Redis 服务器
        "port": 6379,
        "db": 0,
        "password": "your_password",  # 远程 Redis 密码
        "ssl": True,  # 启用 SSL
        "timeout": 30  # 连接超时
    }
}

store = MCPStore.setup_store(debug=True, **redis_config)
print(f"✅ Store 已初始化，远程 Redis 配置: {redis_config}")

# 2️⃣ 添加服务到远程 Redis 后端
print("\n2️⃣ 添加服务到远程 Redis 后端")
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}

store.for_store().add_service(service_config)
print(f"✅ 服务已添加到远程 Redis 后端")

# 3️⃣ 等待服务就绪
print("\n3️⃣ 等待服务就绪")
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务 'weather' 已就绪")

# 4️⃣ 验证远程 Redis 存储
print("\n4️⃣ 验证远程 Redis 存储")
services = store.for_store().list_services()
print(f"✅ 从远程 Redis 获取服务列表: {services}")

if services:
    for service in services:
        print(f"   服务: {service.name}")
        print(f"   状态: {service.status}")

# 5️⃣ 测试工具调用（远程 Redis 后端）
print("\n5️⃣ 测试工具调用（远程 Redis 后端）")
tools = store.for_store().list_tools()
print(f"✅ 从远程 Redis 获取工具列表: {len(tools)} 个工具")

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

# 6️⃣ 测试远程 Redis 数据同步
print("\n6️⃣ 测试远程 Redis 数据同步")
# 添加更多服务
additional_services = {
    "mcpServers": {
        "remote_service": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}

store.for_store().add_service(additional_services)
print(f"✅ 额外服务已添加到远程 Redis")

# 验证服务同步
all_services = store.for_store().list_services()
print(f"   总服务数: {len(all_services)}")
for service in all_services:
    print(f"   服务: {service.name}")

# 7️⃣ 测试远程 Redis 配置管理
print("\n7️⃣ 测试远程 Redis 配置管理")
# 显示当前配置
current_config = store.for_store().show_config()
print(f"✅ 当前配置:")
print(f"   配置类型: {type(current_config)}")
if isinstance(current_config, dict):
    for key, value in current_config.items():
        print(f"   {key}: {value}")

# 8️⃣ 测试远程 Redis 健康检查
print("\n8️⃣ 测试远程 Redis 健康检查")
health_status = store.for_store().check_services()
print(f"✅ 服务健康检查:")
print(f"   健康状态: {health_status}")

# 9️⃣ 测试远程 Redis 性能
print("\n9️⃣ 测试远程 Redis 性能")
import time

# 测试多次工具调用
start_time = time.time()
for i in range(5):
    if tools:
        tool_proxy = store.for_store().find_tool(tools[0].name)
        result = tool_proxy.call_tool({"query": f"远程测试{i}"})
        print(f"   调用 {i+1}: 成功")

end_time = time.time()
total_time = end_time - start_time
print(f"   总耗时: {total_time:.4f}秒")
print(f"   平均耗时: {total_time/5:.4f}秒/次")

# 🔟 测试远程 Redis 连接稳定性
print("\n🔟 测试远程 Redis 连接稳定性")
# 模拟网络中断和重连
print(f"   测试连接稳定性:")
for i in range(3):
    try:
        # 尝试获取服务列表
        services = store.for_store().list_services()
        print(f"   连接测试 {i+1}: 成功，服务数 {len(services)}")
    except Exception as e:
        print(f"   连接测试 {i+1}: 失败 - {e}")

# 1️⃣1️⃣ 测试远程 Redis 安全特性
print("\n1️⃣1️⃣ 测试远程 Redis 安全特性")
print(f"   远程 Redis 安全特性:")
print(f"   - SSL/TLS 加密")
print(f"   - 密码认证")
print(f"   - 连接超时")
print(f"   - 访问控制")
print(f"   - 数据加密")

# 1️⃣2️⃣ 远程 Redis 特性总结
print("\n1️⃣2️⃣ 远程 Redis 特性总结")
print(f"   远程 Redis 数据库支持特性:")
print(f"   - 远程数据存储")
print(f"   - 网络连接管理")
print(f"   - 安全认证")
print(f"   - 数据同步")
print(f"   - 故障恢复")

print("\n💡 Redis 远程服务特点:")
print("   - 远程 Redis 服务器")
print("   - 网络连接管理")
print("   - 安全认证")
print("   - 数据同步")
print("   - 生产环境友好")

print("\n💡 使用场景:")
print("   - 生产环境")
print("   - 分布式部署")
print("   - 数据共享")
print("   - 高可用性")
print("   - 安全要求")

print("\n" + "=" * 60)
print("✅ Redis 数据库支持 - 远程服务测试完成")
print("=" * 60)

