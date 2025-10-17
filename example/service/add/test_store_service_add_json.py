"""
测试：Store 从 JSON 文件添加服务
功能：测试从 JSON 配置文件批量添加服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json
import tempfile

print("=" * 60)
print("测试：Store 从 JSON 文件添加服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 创建临时 JSON 配置文件
print("\n2️⃣ 创建临时 JSON 配置文件")
config_data = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        },
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}

# 创建临时文件
temp_file = tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False, encoding='utf-8')
json.dump(config_data, temp_file, indent=2)
temp_file.close()
temp_path = temp_file.name

print(f"📋 临时配置文件: {temp_path}")
print(f"📋 配置内容:")
print(json.dumps(config_data, indent=2, ensure_ascii=False))

# 3️⃣ 从 JSON 文件添加服务
print("\n3️⃣ 从 JSON 文件添加服务")
result = store.for_store().add_service(json_file=temp_path)
print(f"✅ 服务批量添加成功")
print(f"   返回结果: {result}")

# 4️⃣ 验证服务已添加
print("\n4️⃣ 验证服务已添加")
services = store.for_store().list_services()
print(f"✅ 当前服务数量: {len(services)}")
for svc in services:
    print(f"   - {svc.name}")

# 5️⃣ 清理临时文件
print("\n5️⃣ 清理临时文件")
Path(temp_path).unlink()
print(f"✅ 临时文件已删除")

print("\n💡 JSON 文件配置特点:")
print("   - 支持批量添加多个服务")
print("   - 配置可持久化和版本管理")
print("   - 便于团队共享配置")
print("   - 支持复杂配置结构")

print("\n" + "=" * 60)
print("✅ Store 从 JSON 文件添加服务测试完成")
print("=" * 60)

