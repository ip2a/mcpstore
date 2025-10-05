"""
测试：Store + Redis 初始化
功能：测试 MCPStore.setup_store(redis=...) 的 Redis 配置初始化
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store + Redis 初始化")
print("=" * 60)

# Redis 配置
redis_config = {
    "url": "redis://localhost:6379/0",
    "password": None,
    "namespace": "test_init",
    "dataspace": "auto",
    "socket_timeout": 2.0,
    "healthcheck_interval": 30
}

print("\n📋 Redis 配置:")
for key, value in redis_config.items():
    print(f"   {key}: {value}")

# 1️⃣ 使用 Redis 初始化（新架构：external_db.cache.redis）
print("\n1️⃣ 使用 Redis 初始化（external_db.cache.redis）")
external_db = {"cache": {"type": "redis", **redis_config}}
store = MCPStore.setup_store(debug=True, external_db=external_db)
print(f"✅ Store + Redis 初始化成功: {store}")

# 2️⃣ 验证 Store 可用
print("\n2️⃣ 验证 Store Context 可用")
context = store.for_store()
print(f"✅ Store Context: {context}")

# 3️⃣ 列出服务
print("\n3️⃣ 列出服务")
services = store.for_store().list_services()
print(f"✅ 服务数量: {len(services)}")
if services:
    for svc in services:
        print(f"   - {svc.name}")
else:
    print("   （无服务）")

print("\n💡 提示：")
print("   - 如果 Redis 不可用，MCPStore 会自动回退到内存存储")
print("   - 检查日志可以看到是否成功连接到 Redis")

print("\n" + "=" * 60)
print("✅ Store + Redis 初始化测试完成")
print("=" * 60)

