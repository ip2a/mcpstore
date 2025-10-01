"""
测试：Store 基础初始化
功能：测试 MCPStore.setup_store() 的基础初始化功能
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 基础初始化")
print("=" * 60)

# 1️⃣ 基础初始化（不带任何参数）
print("\n1️⃣ 基础初始化（无参数）")
store = MCPStore.setup_store()
print(f"✅ Store 初始化成功: {store}")
print(f"   类型: {type(store)}")

# 2️⃣ 带 debug 模式初始化
print("\n2️⃣ 带 debug 模式初始化")
store_debug = MCPStore.setup_store(debug=True)
print(f"✅ Debug Store 初始化成功: {store_debug}")

# 3️⃣ 验证 Store 的基础方法可用
print("\n3️⃣ 验证 Store Context 可用")
context = store.for_store()
print(f"✅ Store Context: {context}")
print(f"   类型: {type(context)}")

# 4️⃣ 列出初始服务（应该为空或从配置文件加载）
print("\n4️⃣ 列出初始服务")
services = store.for_store().list_services()
print(f"✅ 初始服务数量: {len(services)}")
if services:
    for svc in services:
        print(f"   - {svc.name}")
else:
    print("   （无服务）")

print("\n" + "=" * 60)
print("✅ Store 基础初始化测试完成")
print("=" * 60)

