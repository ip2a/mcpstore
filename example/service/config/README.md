# 配置管理测试模块

本模块包含 MCPStore 配置管理相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_config_show.py` | Store 显示配置 | Store 级别 |
| `test_store_service_config_reset.py` | Store 重置配置 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 显示配置
python example/service/config/test_store_service_config_show.py

# 重置配置
python example/service/config/test_store_service_config_reset.py
```

### 运行所有配置管理测试

```bash
# Windows
for %f in (example\service\config\test_*.py) do python %f

# Linux/Mac
for f in example/service/config/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 显示配置
测试 `show_config()` 方法：
- 显示全局配置
- 展示配置字段
- 查看服务配置
- 导出配置到文件

### 2. Store 重置配置
测试 `reset_config()` 方法：
- 重置配置到初始状态
- 清除所有服务
- 对比重置前后
- 重新添加服务

## 💡 核心概念

### show_config() vs reset_config()

| 方法 | 操作类型 | 影响 | 返回值 | 使用场景 |
|------|----------|------|--------|----------|
| **show_config()** | 查询 | 无影响 | 配置字典 | 查看、导出配置 |
| **reset_config()** | 修改 | 清除所有配置 | 布尔值 | 重置、清理环境 |

### show_config() 方法签名

```python
def show_config() -> dict:
    """
    显示 MCPStore 的全局配置
    
    返回:
        dict: 配置字典，包含所有配置信息
        
    配置内容:
        - mcpServers: 已注册的服务配置
        - debug: 调试模式
        - workspace: 工作空间路径
        - dataspace: 数据空间标识
        - redis: Redis 配置（如果启用）
    """
```

### reset_config() 方法签名

```python
def reset_config() -> bool:
    """
    重置 MCPStore 配置到初始状态
    
    返回:
        bool: 重置是否成功
        
    影响:
        - 清除所有服务配置
        - 停止所有运行中的服务
        - 恢复默认设置
        - 清理缓存
    """
```

## 🎯 使用场景

### 场景 1：查看当前配置
```python
# 查看完整配置
config = store.for_store().show_config()
print(json.dumps(config, indent=2))

# 检查特定配置
if 'mcpServers' in config:
    print(f"已注册服务: {list(config['mcpServers'].keys())}")
```

### 场景 2：导出配置备份
```python
import json

# 导出配置
config = store.for_store().show_config()
with open('backup_config.json', 'w', encoding='utf-8') as f:
    json.dump(config, f, indent=2, ensure_ascii=False)
print("配置已备份")
```

### 场景 3：重置测试环境
```python
# 测试前重置环境
store.for_store().reset_config()
print("测试环境已重置")

# 添加测试服务
store.for_store().add_service(test_config)
```

### 场景 4：配置迁移
```python
# 从旧环境导出
old_config = old_store.for_store().show_config()

# 在新环境导入
new_store.for_store().reset_config()
for service_name, service_config in old_config['mcpServers'].items():
    new_store.for_store().add_service({
        "mcpServers": {
            service_name: service_config
        }
    })
```

## 📊 配置结构

### 典型配置结构

```json
{
  "mcpServers": {
    "weather": {
      "url": "https://mcpstore.wiki/mcp"
    },
    "search": {
      "command": "npx",
      "args": ["-y", "search-mcp"]
    }
  },
  "debug": true,
  "workspace": "/path/to/workspace",
  "dataspace": "auto",
  "redis": {
    "url": "redis://localhost:6379/0",
    "namespace": "default"
  }
}
```

### 配置字段说明

| 字段 | 类型 | 说明 | 示例 |
|------|------|------|------|
| `mcpServers` | object | 已注册的服务配置 | `{"weather": {...}}` |
| `debug` | boolean | 调试模式 | `true` / `false` |
| `workspace` | string | 工作空间路径 | `"/path/to/workspace"` |
| `dataspace` | string | 数据空间标识 | `"auto"` / `"workspace1"` |
| `redis` | object | Redis 配置 | `{"url": "..."}` |

## 💡 最佳实践

### 1. 定期备份配置
```python
import json
from datetime import datetime

def backup_config(store):
    """定期备份配置"""
    config = store.for_store().show_config()
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    filename = f"config_backup_{timestamp}.json"
    
    with open(filename, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=2, ensure_ascii=False)
    
    print(f"配置已备份到: {filename}")
```

### 2. 重置前确认
```python
def safe_reset(store):
    """安全重置配置"""
    # 显示当前配置
    config = store.for_store().show_config()
    services = store.for_store().list_services()
    
    print(f"即将重置配置")
    print(f"当前服务数量: {len(services)}")
    print(f"服务列表: {[s.name for s in services]}")
    
    # 确认
    confirm = input("确认重置？(yes/no): ")
    if confirm.lower() == 'yes':
        # 备份
        backup_config(store)
        
        # 重置
        store.for_store().reset_config()
        print("✅ 配置已重置")
    else:
        print("❌ 取消重置")
```

### 3. 配置版本控制
```python
# .gitignore 中排除敏感信息
"""
mcp_config.json
config_*.json
!config_template.json
"""

# 使用配置模板
config_template = {
    "mcpServers": {
        "example": {
            "url": "${SERVICE_URL}"  # 使用环境变量
        }
    }
}
```

### 4. 环境区分
```python
import os

def load_config_for_env():
    """根据环境加载配置"""
    env = os.getenv('ENV', 'development')
    
    if env == 'production':
        config_file = 'config_prod.json'
    elif env == 'staging':
        config_file = 'config_staging.json'
    else:
        config_file = 'config_dev.json'
    
    with open(config_file, 'r') as f:
        return json.load(f)
```

## 🔧 常见问题

### Q1: show_config() 包含敏感信息吗？
**A**: 可能包含。建议：
- 不要将配置文件提交到公开仓库
- 使用环境变量存储敏感信息
- 导出时过滤敏感字段

### Q2: reset_config() 会删除配置文件吗？
**A**: 取决于实现。通常：
- 清除内存中的配置
- 可能清除配置文件
- 建议先备份

### Q3: 重置后能恢复吗？
**A**: 如果有备份可以恢复：
```python
# 备份
backup = store.for_store().show_config()

# 重置
store.for_store().reset_config()

# 恢复
for name, cfg in backup['mcpServers'].items():
    store.for_store().add_service({"mcpServers": {name: cfg}})
```

### Q4: 配置存储在哪里？
**A**: 通常存储在：
- 内存中（运行时）
- 配置文件（如 `mcp.json`）
- Redis（如果启用）
- 工作空间目录

## ⚠️ 警告事项

### show_config()
- ⚠️ 可能包含敏感信息
- ⚠️ 不要公开分享配置
- ✅ 适合本地查看和备份

### reset_config()
- ⚠️ 操作不可逆
- ⚠️ 所有服务会被停止
- ⚠️ 配置会被清除
- ✅ 使用前先备份

## 🔗 相关文档

- [show_config() 文档](../../../mcpstore_docs/docs/services/config/show-config.md)
- [reset_config() 文档](../../../mcpstore_docs/docs/services/config/reset-config.md)
- [配置格式说明](../../../mcpstore_docs/docs/services/registration/config-formats.md)
- [MCPStore 类文档](../../../mcpstore_docs/docs/api-reference/mcpstore-class.md)

