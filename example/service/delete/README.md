# 删除服务测试模块

本模块包含服务删除相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_delete_remove.py` | Store 移除服务（运行态） | Store 级别 |
| `test_store_service_delete_full.py` | Store 完全删除服务 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 移除服务（运行态）
python example/service/delete/test_store_service_delete_remove.py

# 完全删除服务
python example/service/delete/test_store_service_delete_full.py
```

### 运行所有删除服务测试

```bash
# Windows
for %f in (example\service\delete\test_*.py) do python %f

# Linux/Mac
for f in example/service/delete/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 移除服务（运行态）
测试 `remove_service()` 方法：
- 移除服务的运行实例
- 验证服务已从列表移除
- 尝试查找已移除的服务
- 重新添加服务
- 选择性移除多个服务

### 2. Store 完全删除服务
测试 `delete_service()` 方法：
- 完全删除服务
- 删除配置和缓存
- 验证服务已彻底删除
- 重新添加同名服务
- 对比 remove 和 delete
- 批量删除

## 💡 核心概念

### remove_service() vs delete_service()

| 方法 | 删除范围 | 配置文件 | 缓存数据 | 可恢复性 | 使用场景 |
|------|----------|----------|----------|----------|----------|
| **remove_service()** | 运行实例 | 可能保留 | 可能保留 | 可快速恢复 | 临时停止、释放资源 |
| **delete_service()** | 完全删除 | 删除 | 删除 | 不可恢复 | 永久移除、彻底清理 |

### remove_service() 方法签名

```python
def remove_service() -> bool:
    """
    移除服务的运行实例
    
    返回:
        bool: 移除是否成功
    
    说明:
        - 停止服务进程
        - 从运行列表中移除
        - 配置文件可能保留
        - 可以重新添加服务
    """
```

### delete_service() 方法签名

```python
def delete_service() -> bool:
    """
    完全删除服务
    
    返回:
        bool: 删除是否成功
    
    说明:
        - 停止服务进程
        - 删除配置文件
        - 删除所有缓存
        - 彻底清除，不可恢复
    """
```

## 🎯 使用场景

### 场景 1：临时停止服务（remove）
```python
service = store.for_store().find_service("weather")

# 临时停止服务
service.remove_service()
print("服务已临时停止")

# 稍后可以重新添加
store.for_store().add_service({
    "mcpServers": {
        "weather": {"url": "..."}
    }
})
```

### 场景 2：永久移除服务（delete）
```python
service = store.for_store().find_service("weather")

# 永久删除服务
service.delete_service()
print("服务已永久删除，包括所有配置和缓存")
```

### 场景 3：服务升级
```python
service = store.for_store().find_service("weather")

# 方法1：remove + 重新添加
service.remove_service()
store.for_store().add_service(new_config)

# 方法2：delete + 重新添加（更彻底）
service.delete_service()
store.for_store().add_service(new_config)
```

### 场景 4：批量清理
```python
# 清理所有测试服务
services = store.for_store().list_services()
for svc in services:
    if svc.name.startswith("test_"):
        service = store.for_store().find_service(svc.name)
        service.delete_service()
        print(f"已删除测试服务: {svc.name}")
```

## 📊 删除对比

### remove_service() 操作流程

```
remove_service()
    ↓
1. 停止服务进程
    ↓
2. 从运行列表移除
    ↓
3. 释放运行时资源
    ↓
4. 完成（配置保留）
```

### delete_service() 操作流程

```
delete_service()
    ↓
1. 停止服务进程
    ↓
2. 从运行列表移除
    ↓
3. 删除配置文件
    ↓
4. 删除缓存数据
    ↓
5. 完成（彻底清除）
```

## 💡 最佳实践

### 1. 删除前备份配置
```python
service = store.for_store().find_service("weather")

# 备份配置
config_backup = service.service_info()['config']

# 删除服务
service.delete_service()

# 如果需要，可以用备份恢复
# store.for_store().add_service({
#     "mcpServers": {
#         "weather": config_backup
#     }
# })
```

### 2. 删除前检查依赖
```python
def safe_delete(service_name):
    """安全删除服务"""
    # 检查是否有其他服务依赖
    # 这里只是示例，实际需要根据业务逻辑实现
    
    service = store.for_store().find_service(service_name)
    
    # 确认删除
    print(f"即将删除服务: {service_name}")
    print("此操作不可逆，请确认")
    
    # 执行删除
    service.delete_service()
    print(f"✅ 已删除: {service_name}")
```

### 3. 区分使用场景
```python
# ✅ 临时停止：使用 remove
if need_temp_stop:
    service.remove_service()

# ✅ 永久移除：使用 delete
if need_permanent_delete:
    service.delete_service()
```

### 4. 批量删除错误处理
```python
def batch_delete(service_names):
    """批量删除服务"""
    results = {
        'success': [],
        'failed': []
    }
    
    for name in service_names:
        try:
            service = store.for_store().find_service(name)
            service.delete_service()
            results['success'].append(name)
            print(f"✅ {name} 删除成功")
        except Exception as e:
            results['failed'].append((name, str(e)))
            print(f"❌ {name} 删除失败: {e}")
    
    return results
```

## 🔧 常见问题

### Q1: remove 后配置还在吗？
**A**: 可能在，取决于实现。建议：
- 需要保留配置：使用 `remove_service()`
- 不需要保留：使用 `delete_service()`

### Q2: delete 后能恢复吗？
**A**: 不能。`delete_service()` 是永久删除，不可恢复。删除前请确保备份重要配置。

### Q3: 删除服务会影响其他服务吗？
**A**: 不会。每个服务是独立的，删除一个不影响其他服务。

### Q4: 如何批量删除所有服务？
**A**: 
```python
services = store.for_store().list_services()
for svc in services:
    service = store.for_store().find_service(svc.name)
    service.delete_service()
```

### Q5: 删除正在使用的服务会怎样？
**A**: 
- 服务会立即停止
- 正在进行的工具调用会失败
- 建议在低峰期或确认无调用时删除

## ⚠️ 警告事项

### remove_service()
- ⚠️ 服务立即不可用
- ⚠️ 正在进行的调用会失败
- ✅ 可以重新添加

### delete_service()
- ⚠️ 操作不可逆
- ⚠️ 配置和缓存全部删除
- ⚠️ 无法恢复
- ✅ 彻底清理

## 🔗 相关文档

- [remove_service() 文档](../../../mcpstore_docs/docs/services/management/remove-service.md)
- [delete_service() 文档](../../../mcpstore_docs/docs/services/management/delete-service.md)
- [服务生命周期](../../../mcpstore_docs/docs/advanced/lifecycle.md)
- [ServiceProxy 文档](../../../mcpstore_docs/docs/services/listing/service-proxy.md)

