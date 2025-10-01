# 重启服务测试模块

本模块包含服务重启和刷新相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_restart_basic.py` | Store 重启服务 | Store 级别 |
| `test_store_service_restart_refresh.py` | Store 刷新服务内容 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 重启服务
python example/service/restart/test_store_service_restart_basic.py

# 刷新服务内容
python example/service/restart/test_store_service_restart_refresh.py
```

### 运行所有重启服务测试

```bash
# Windows
for %f in (example\service\restart\test_*.py) do python %f

# Linux/Mac
for f in example/service/restart/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 重启服务
测试 `restart_service()` 方法：
- 重启服务进程
- 对比重启前后的状态
- 等待服务重新就绪
- 验证服务可用
- 测试多次重启

### 2. Store 刷新服务内容
测试 `refresh_content()` 方法：
- 刷新服务的工具列表
- 对比刷新前后的工具
- 验证服务状态
- 对比 refresh 和 restart
- 测试多次刷新

## 💡 核心概念

### restart_service() vs refresh_content()

| 方法 | 操作范围 | 影响程度 | 耗时 | 服务中断 | 使用场景 |
|------|----------|----------|------|----------|----------|
| **restart_service()** | 完全重启 | 重启进程 | 长 | 是 | 服务异常、重大配置变更 |
| **refresh_content()** | 刷新内容 | 更新列表 | 短 | 否 | 工具列表更新、轻量同步 |

### restart_service() 方法签名

```python
def restart_service() -> bool:
    """
    重启服务
    
    返回:
        bool: 重启是否成功
    
    说明:
        - 停止当前服务进程
        - 重新启动服务
        - 重新建立连接
        - 重新加载配置
    """
```

### refresh_content() 方法签名

```python
def refresh_content() -> bool:
    """
    刷新服务内容
    
    返回:
        bool: 刷新是否成功
    
    说明:
        - 重新获取工具列表
        - 重新获取资源列表
        - 重新获取提示列表
        - 服务进程保持运行
    """
```

## 🎯 使用场景

### 场景 1：服务异常时重启
```python
service = store.for_store().find_service("weather")

# 检查服务健康
health = service.check_health()
if health['status'] != 'healthy':
    print("⚠️ 服务不健康，尝试重启...")
    service.restart_service()
    store.for_store().wait_service("weather", timeout=30.0)
    print("✅ 服务已重启")
```

### 场景 2：配置更新后重启
```python
service = store.for_store().find_service("weather")

# 更新配置
service.update_config({
    "url": "https://new-api.com/mcp",
    "timeout": 90
})

# 重启服务使配置生效
service.restart_service()
store.for_store().wait_service("weather", timeout=30.0)
```

### 场景 3：刷新工具列表
```python
service = store.for_store().find_service("weather")

# 轻量级刷新，获取最新工具列表
service.refresh_content()

# 立即可用，无需等待
tools = service.list_tools()
print(f"最新工具数量: {len(tools)}")
```

### 场景 4：定期维护
```python
import time
import schedule

def maintenance_restart():
    """定期维护重启"""
    services = store.for_store().list_services()
    for svc in services:
        service = store.for_store().find_service(svc.name)
        print(f"维护重启: {svc.name}")
        service.restart_service()
        store.for_store().wait_service(svc.name, timeout=30.0)

# 每天凌晨3点重启
schedule.every().day.at("03:00").do(maintenance_restart)
```

## 📊 操作对比

### 重启服务流程

```
restart_service()
    ↓
1. 停止服务进程
    ↓
2. 清理资源
    ↓
3. 重新启动进程
    ↓
4. 重新建立连接
    ↓
5. 重新加载配置
    ↓
6. 服务就绪
```

### 刷新内容流程

```
refresh_content()
    ↓
1. 连接到服务（不重启）
    ↓
2. 请求最新工具列表
    ↓
3. 请求最新资源列表
    ↓
4. 请求最新提示列表
    ↓
5. 更新本地缓存
    ↓
6. 完成（服务持续运行）
```

## 💡 最佳实践

### 1. 重启前备份状态
```python
service = store.for_store().find_service("weather")

# 备份状态
status_before = service.service_status()
config_before = service.service_info()['config']

# 重启
service.restart_service()

# 验证
store.for_store().wait_service("weather", timeout=30.0)
status_after = service.service_status()
print(f"重启前状态: {status_before['state']}")
print(f"重启后状态: {status_after['state']}")
```

### 2. 优先使用 refresh
```python
# ✅ 推荐：优先尝试轻量级刷新
service = store.for_store().find_service("weather")
service.refresh_content()

# 如果刷新不够，再考虑重启
if still_has_issues:
    service.restart_service()
```

### 3. 重启后完整验证
```python
def restart_and_verify(service_name):
    service = store.for_store().find_service(service_name)
    
    # 重启
    service.restart_service()
    
    # 等待就绪
    store.for_store().wait_service(service_name, timeout=30.0)
    
    # 完整验证
    health = service.check_health()
    assert health['status'] == 'healthy', "重启后服务不健康"
    
    tools = service.list_tools()
    assert len(tools) > 0, "重启后无工具"
    
    print(f"✅ {service_name} 重启并验证成功")
```

### 4. 批量重启策略
```python
def restart_all_services():
    """批量重启所有服务"""
    services = store.for_store().list_services()
    
    for svc in services:
        try:
            print(f"重启 {svc.name}...")
            service = store.for_store().find_service(svc.name)
            service.restart_service()
            store.for_store().wait_service(svc.name, timeout=30.0)
            print(f"✅ {svc.name} 重启成功")
        except Exception as e:
            print(f"❌ {svc.name} 重启失败: {e}")
```

## 🔧 常见问题

### Q1: restart_service() 需要多长时间？
**A**: 取决于服务类型：
- 本地服务：5-15秒
- 远程服务：10-30秒
- 复杂服务：30-60秒

### Q2: 重启会丢失什么？
**A**: 
- ✅ 配置不会丢失（持久化）
- ❌ 运行时状态会重置
- ❌ 内存中的临时数据会丢失
- ❌ 运行时间计数器重置

### Q3: refresh_content() 会影响正在进行的工具调用吗？
**A**: 不会。`refresh_content()` 只更新本地缓存的工具列表，不影响正在执行的工具。

### Q4: 如何判断应该用 restart 还是 refresh？
**A**: 
```python
# 决策流程
if 服务异常 or 配置重大变更:
    use restart_service()
elif 工具列表需要更新:
    use refresh_content()
else:
    不需要操作
```

## 🔗 相关文档

- [restart_service() 文档](../../../mcpstore_docs/docs/services/management/restart-service.md)
- [refresh_content() 文档](../../../mcpstore_docs/docs/services/management/refresh-content.md)
- [服务生命周期](../../../mcpstore_docs/docs/advanced/lifecycle.md)
- [ServiceProxy 文档](../../../mcpstore_docs/docs/services/listing/service-proxy.md)

