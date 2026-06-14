#!/usr/bin/env python3
"""
示例 14：多 Agent 数据隔离
功能：演示 Agent 之间的数据完全隔离
特点：Collection 映射，互不干扰
"""

import sys
from pathlib import Path

EXAMPLE_DIR = Path(__file__).resolve().parents[2]
if str(EXAMPLE_DIR) not in sys.path:
    sys.path.insert(0, str(EXAMPLE_DIR))

from example_utils import setup_import_path

setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("示例 14：多 Agent 数据隔离")
print("=" * 60)

# 初始化 Store
store = MCPStore.setup_store(
    mcpjson_path="./mcp.json",
    debug=False
)

print("\n Store 初始化成功")

# Agent 1：添加服务
print("\n" + "=" * 60)
print("Agent 1 操作")
print("=" * 60)

store.for_agent("agent_001").add_service({
    "mcpServers": {
        "weather": {
            "command": "echo",
            "args": ["weather"]
        }
    }
})
print(" Agent 1 添加了 weather 服务")

store.for_agent("agent_001").add_service({
    "mcpServers": {
        "calendar": {
            "command": "echo",
            "args": ["calendar"]
        }
    }
})
print(" Agent 1 添加了 calendar 服务")

services_1 = store.for_agent("agent_001").list_services()
print(f"\n📋 Agent 1 的服务: {len(services_1)} 个")
for service in services_1:
    print(f"   - {service}")

# Agent 2：添加不同的服务
print("\n" + "=" * 60)
print("Agent 2 操作")
print("=" * 60)

store.for_agent("agent_002").add_service({
    "mcpServers": {
        "database": {
            "command": "echo",
            "args": ["database"]
        }
    }
})
print(" Agent 2 添加了 database 服务")

store.for_agent("agent_002").add_service({
    "mcpServers": {
        "email": {
            "command": "echo",
            "args": ["email"]
        }
    }
})
print(" Agent 2 添加了 email 服务")

services_2 = store.for_agent("agent_002").list_services()
print(f"\n📋 Agent 2 的服务: {len(services_2)} 个")
for service in services_2:
    print(f"   - {service}")

# Agent 3：添加更多服务
print("\n" + "=" * 60)
print("Agent 3 操作")
print("=" * 60)

store.for_agent("agent_003").add_service({
    "mcpServers": {
        "filesystem": {
            "command": "echo",
            "args": ["filesystem"]
        }
    }
})
print(" Agent 3 添加了 filesystem 服务")

services_3 = store.for_agent("agent_003").list_services()
print(f"\n📋 Agent 3 的服务: {len(services_3)} 个")
for service in services_3:
    print(f"   - {service}")

# 验证隔离
print("\n" + "=" * 60)
print("验证数据隔离")
print("=" * 60)

print("\n检查 Agent 之间的数据隔离：")
print(f"   Agent 1 服务: {services_1}")
print(f"   Agent 2 服务: {services_2}")
print(f"   Agent 3 服务: {services_3}")

# 验证没有交叉
if (set(services_1) & set(services_2)) == set():
    print("\n Agent 1 和 Agent 2 数据完全隔离")
else:
    print("\n⚠️  警告：Agent 1 和 Agent 2 有数据交叉")

if (set(services_2) & set(services_3)) == set():
    print(" Agent 2 和 Agent 3 数据完全隔离")
else:
    print("⚠️  警告：Agent 2 和 Agent 3 有数据交叉")

if (set(services_1) & set(services_3)) == set():
    print(" Agent 1 和 Agent 3 数据完全隔离")
else:
    print("⚠️  警告：Agent 1 和 Agent 3 有数据交叉")

# Store 级别的服务
print("\n" + "=" * 60)
print("Store 级别（全局）")
print("=" * 60)

store.for_store().add_service({
    "mcpServers": {
        "global_service": {
            "command": "echo",
            "args": ["global"]
        }
    }
})
print(" Store 添加了 global_service")

store_services = store.for_store().list_services()
print(f"\n📋 Store 的服务: {len(store_services)} 个")
for service in store_services:
    print(f"   - {service}")

# 验证 Store 和 Agent 隔离
if (set(store_services) & set(services_1)) == set():
    print("\n Store 和 Agent 数据完全隔离")
else:
    print("\n⚠️  警告：Store 和 Agent 有数据交叉")

# 清理
store.for_agent("agent_001").reset_config()
store.for_agent("agent_002").reset_config()
store.for_agent("agent_003").reset_config()
store.for_store().reset_config()
print("\n 所有配置已重置")

print("\n" + "=" * 60)
print("数据隔离原理：")
print("=" * 60)
print("\n📦 Collection 映射策略：")
print("   Store:    agent:global_agent_store:tools")
print("   Agent 1:  agent:agent_001:tools")
print("   Agent 2:  agent:agent_002:tools")
print("   Agent 3:  agent:agent_003:tools")
print("\n 每个 Agent 使用独立的 Collection")
print(" 数据在存储层面完全隔离")
print(" 互不干扰，安全可靠")

print("\n💡 使用场景：")
print("   - 多租户 SaaS 应用")
print("   - 多用户隔离")
print("   - 开发/测试环境隔离")
print("   - 不同业务线隔离")
