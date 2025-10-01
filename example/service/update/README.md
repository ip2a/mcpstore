# 更新服务测试模块

本模块包含服务配置更新相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_update_full.py` | Store 完整更新服务配置 | Store 级别 |
| `test_store_service_update_patch.py` | Store 增量更新服务配置 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 完整更新服务配置
python example/service/update/test_store_service_update_full.py

# 增量更新服务配置
python example/service/update/test_store_service_update_patch.py
```

### 运行所有更新服务测试

```bash
# Windows
for %f in (example\service\update\test_*.py) do python %f

# Linux/Mac
for f in example/service/update/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 完整更新服务配置
测试 `update_config()` 方法：
- 完整替换服务配置
- 获取更新前后的配置对比
- 验证服务仍然可用
- 测试多次更新

### 2. Store 增量更新服务配置
测试 `patch_config()` 方法：
- 增量更新服务配置
- 只修改指定字段
- 保留原有字段
- 添加新字段
- 修改已存在的字段

## 💡 核心概念

### update_config() vs patch_config()

| 方法 | 更新方式 | 字段处理 | 用途 | 使用场景 |
|------|----------|----------|------|----------|
| **update_config()** | 完整替换 | 旧字段会被删除 | 重新配置 | 切换环境、大改动 |
| **patch_config()** | 增量更新 | 旧字段保留 | 微调配置 | 调整参数、小改动 |

### update_config() 方法签名

```python
def update_config(new_config: dict) -> bool:
    """
    完整更新服务配置
    
    参数:
        new_config: 新的完整配置
    
    返回:
        bool: 更新是否成功
    """
```

### patch_config() 方法签名

```python
def patch_config(patch: dict) -> bool:
    """
    增量更新服务配置
    
    参数:
        patch: 要更新的字段（部分配置）
    
    返回:
        bool: 更新是否成功
    """
```

## 🎯 使用场景

### 场景 1：切换环境（完整更新）
```python
service = store.for_store().find_service("weather")

# 开发环境配置
dev_config = {
    "url": "http://localhost:3000/mcp",
    "timeout": 10,
    "debug": True
}

# 生产环境配置
prod_config = {
    "url": "https://api.prod.com/mcp",
    "timeout": 60,
    "retry": 3,
    "cache": True
}

# 切换到生产环境
service.update_config(prod_config)
```

### 场景 2：调整超时时间（增量更新）
```python
service = store.for_store().find_service("weather")

# 只修改超时时间，其他配置保持不变
service.patch_config({"timeout": 90})
```

### 场景 3：动态调整配置
```python
service = store.for_store().find_service("weather")

# 根据运行情况动态调整
if performance_issues:
    service.patch_config({
        "timeout": 120,
        "retry": 5
    })
elif memory_issues:
    service.patch_config({
        "cache": False
    })
```

### 场景 4：配置迁移
```python
# 从旧配置迁移到新配置
old_config = service.service_info()['config']

# 构建新配置
new_config = {
    "url": migrate_url(old_config['url']),
    "timeout": old_config.get('timeout', 30) * 2,
    "new_feature": True
}

# 完整更新
service.update_config(new_config)
```

## 📊 配置更新示例

### 完整更新示例

```python
# 初始配置
{
    "url": "https://old.com/mcp",
    "timeout": 30
}

# 使用 update_config()
service.update_config({
    "url": "https://new.com/mcp",
    "timeout": 60,
    "retry": 3
})

# 结果：完全替换
{
    "url": "https://new.com/mcp",
    "timeout": 60,
    "retry": 3
}
# 注意：原有的字段都被新配置替换
```

### 增量更新示例

```python
# 初始配置
{
    "url": "https://api.com/mcp",
    "timeout": 30
}

# 使用 patch_config()
service.patch_config({
    "timeout": 60,
    "retry": 3
})

# 结果：增量合并
{
    "url": "https://api.com/mcp",  # 保留
    "timeout": 60,                  # 修改
    "retry": 3                      # 新增
}
# 注意：原有字段保留，只修改指定字段
```

## 💡 最佳实践

### 1. 备份原配置
```python
# 更新前备份
service = store.for_store().find_service("weather")
backup_config = service.service_info()['config'].copy()

try:
    service.update_config(new_config)
except Exception as e:
    # 恢复配置
    service.update_config(backup_config)
    print(f"配置更新失败，已恢复: {e}")
```

### 2. 验证新配置
```python
def update_with_validation(service, new_config):
    # 备份
    old_config = service.service_info()['config']
    
    # 更新
    service.update_config(new_config)
    
    # 验证服务可用
    try:
        store.for_store().wait_service(service_name, timeout=10.0)
        print("✅ 配置更新成功，服务正常")
    except Exception as e:
        # 回滚
        service.update_config(old_config)
        print(f"⚠️ 配置更新失败，已回滚: {e}")
```

### 3. 增量更新优先
```python
# ✅ 推荐：使用增量更新
service.patch_config({"timeout": 90})

# ❌ 不推荐：为了改一个字段用完整更新
service.update_config({
    "url": "...",      # 需要重新写所有字段
    "timeout": 90,     # 只是想改这个
    "retry": 3,
    # ... 其他所有字段
})
```

### 4. 更新后检查
```python
# 更新配置
service.patch_config({"timeout": 90})

# 立即验证
updated_config = service.service_info()['config']
assert updated_config['timeout'] == 90, "配置更新失败"

# 检查服务状态
status = service.service_status()
print(f"服务状态: {status['state']}")
```

## 🔧 常见问题

### Q1: update_config() 后服务需要重启吗？
**A**: 取决于配置类型：
- URL 变更：通常需要重启
- 超时/重试：可能不需要重启
- 建议：更新后使用 `wait_service()` 确保服务可用

### Q2: patch_config() 可以删除字段吗？
**A**: 不能。`patch_config()` 只能添加或修改字段，不能删除。如需删除字段，使用 `update_config()`。

### Q3: 配置更新会影响正在使用的工具吗？
**A**: 可能会。建议：
- 在低峰期更新配置
- 更新前通知用户
- 更新后验证服务可用

### Q4: 如何批量更新多个服务的配置？
**A**: 
```python
services = ["service1", "service2", "service3"]
new_config = {"timeout": 90}

for svc_name in services:
    service = store.for_store().find_service(svc_name)
    service.patch_config(new_config)
    print(f"✅ {svc_name} 配置已更新")
```

## 🔗 相关文档

- [update_config() 文档](../../../mcpstore_docs/docs/services/management/update-service.md)
- [patch_config() 文档](../../../mcpstore_docs/docs/services/management/patch-service.md)
- [服务配置格式](../../../mcpstore_docs/docs/services/registration/config-formats.md)
- [ServiceProxy 文档](../../../mcpstore_docs/docs/services/listing/service-proxy.md)

