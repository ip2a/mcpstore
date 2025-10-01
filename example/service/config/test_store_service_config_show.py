"""
测试：Store 显示配置
功能：测试使用 show_config() 显示 MCPStore 的全局配置
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
print("测试：Store 显示配置")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=False)
print(f"✅ Store 初始化成功")

# 2️⃣ 使用 show_config() 显示全局配置
print("\n2️⃣ 使用 show_config() 显示全局配置")
config = store.for_store().show_config()
print(f"✅ 全局配置获取成功")
print(f"   返回类型: {type(config)}")

# 3️⃣ 展示配置的主要字段
print("\n3️⃣ 展示配置的主要字段")
if isinstance(config, dict):
    print(f"📋 全局配置字段:")
    for key in config.keys():
        print(f"   - {key}")

# 4️⃣ 展示完整配置（JSON 格式）
print("\n4️⃣ 完整配置（JSON 格式）:")
print("-" * 60)
print(json.dumps(config, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 5️⃣ 添加服务后查看配置变化
print("\n5️⃣ 添加服务后查看配置变化")
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"✅ 已添加服务 'weather'")

config_after = store.for_store().show_config()
print(f"📋 添加服务后的配置:")
if 'mcpServers' in config_after:
    print(f"   mcpServers: {list(config_after['mcpServers'].keys())}")

# 6️⃣ 检查特定配置项
print("\n6️⃣ 检查特定配置项")
if isinstance(config_after, dict):
    if 'mcpServers' in config_after:
        print(f"   ✅ 包含 mcpServers 配置")
        print(f"      服务数量: {len(config_after['mcpServers'])}")
    
    if 'debug' in config_after:
        print(f"   ✅ Debug 模式: {config_after['debug']}")
    
    if 'workspace' in config_after:
        print(f"   ✅ 工作空间: {config_after['workspace']}")

# 7️⃣ 配置的用途说明
print("\n7️⃣ 配置包含的信息")
print(f"   全局配置通常包含:")
print(f"   - mcpServers: 已注册的服务配置")
print(f"   - debug: 调试模式开关")
print(f"   - workspace: 工作空间路径")
print(f"   - dataspace: 数据空间标识")
print(f"   - redis: Redis 配置（如果启用）")
print(f"   - 其他全局设置")

# 8️⃣ 导出配置到文件示例
print("\n8️⃣ 导出配置到文件示例")
import tempfile
temp_file = tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False, encoding='utf-8')
json.dump(config_after, temp_file, indent=2, ensure_ascii=False, default=str)
temp_file.close()
print(f"✅ 配置已导出到临时文件: {temp_file.name}")
print(f"   （实际使用中可以导出到指定路径）")

# 清理临时文件
Path(temp_file.name).unlink()
print(f"✅ 临时文件已清理")

print("\n💡 show_config() 特点:")
print("   - 显示 MCPStore 的全局配置")
print("   - 包含所有已注册的服务")
print("   - 包含全局设置和参数")
print("   - 返回完整的配置字典")
print("   - 适合配置查看和导出")

print("\n💡 使用场景:")
print("   - 查看当前配置")
print("   - 导出配置备份")
print("   - 配置调试")
print("   - 配置迁移")
print("   - 团队共享配置")

print("\n💡 配置管理建议:")
print("   - 定期备份配置")
print("   - 使用版本控制管理配置文件")
print("   - 敏感信息不要硬编码")
print("   - 区分开发和生产配置")

print("\n" + "=" * 60)
print("✅ Store 显示配置测试完成")
print("=" * 60)

