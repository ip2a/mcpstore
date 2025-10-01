# 健康检查测试模块

本模块包含服务健康检查相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_health_all.py` | Store 检查所有服务健康状态 | Store 级别 |
| `test_store_service_health_single.py` | Store 检查单个服务健康状态 | Store 级别 |
| `test_store_service_health_details.py` | Store 获取服务详细健康信息 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 检查所有服务健康状态
python example/service/health/test_store_service_health_all.py

# 检查单个服务健康状态
python example/service/health/test_store_service_health_single.py

# 获取服务详细健康信息
python example/service/health/test_store_service_health_details.py
```

### 运行所有健康检查测试

```bash
# Windows
for %f in (example\service\health\test_*.py) do python %f

# Linux/Mac
for f in example/service/health/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 检查所有服务健康状态
测试 `check_services()` 方法：
- 检查所有已注册服务
- 返回聚合的健康报告
- 展示总数、健康数、不健康数
- 展示每个服务的健康状态
- 判断整体健康状态

### 2. Store 检查单个服务健康状态
测试 `check_health()` 方法：
- 检查单个服务的健康状态
- 返回健康摘要
- 对比多个服务的健康状态
- 判断服务是否健康

### 3. Store 获取服务详细健康信息
测试 `health_details()` 方法：
- 获取最详细的健康信息
- 展示错误和警告列表
- 展示工具、资源、提示数量
- 对比三种健康检查方法
- 使用详细信息进行诊断

## 💡 核心概念

### 三种健康检查方法

| 方法 | 级别 | 详细程度 | 用途 | 调用方式 |
|------|------|----------|------|----------|
| `check_services()` | Context | 聚合报告 | 所有服务整体健康 | `store.for_store().check_services()` |
| `check_health()` | ServiceProxy | 健康摘要 | 单个服务快速检查 | `service_proxy.check_health()` |
| `health_details()` | ServiceProxy | 详细信息 | 单个服务深度诊断 | `service_proxy.health_details()` |

### check_services() 返回结构

```python
health_report = store.for_store().check_services()

# 结构示例
{
    "total": 3,                    # 总服务数
    "healthy": 2,                  # 健康服务数
    "unhealthy": 1,                # 不健康服务数
    "services": {
        "weather": {
            "status": "healthy",
            "state": "running",
            "last_check": "2025-01-09..."
        },
        "search": {
            "status": "healthy",
            "state": "running"
        }
    }
}
```

### check_health() 返回结构

```python
health_summary = service_proxy.check_health()

# 结构示例
{
    "status": "healthy",           # 健康状态
    "state": "running",            # 生命周期状态
    "connected": true,             # 连接状态
    "message": "Service is healthy"
}
```

### health_details() 返回结构

```python
health_details = service_proxy.health_details()

# 结构示例
{
    "status": "healthy",
    "state": "running",
    "connected": true,
    "health": "healthy",
    "last_check": "2025-01-09...",
    "uptime": 3600,
    "errors": [],                  # 错误列表
    "warnings": [],                # 警告列表
    "tools_count": 5,              # 工具数量
    "resources_count": 0,          # 资源数量
    "prompts_count": 0             # 提示数量
}
```

## 🎯 使用场景

### 场景 1：整体健康监控
```python
# 监控所有服务
health = store.for_store().check_services()
if health['healthy'] == health['total']:
    print("✅ 所有服务健康")
else:
    print(f"⚠️ {health['unhealthy']} 个服务不健康")
```

### 场景 2：单个服务快速检查
```python
# 快速检查特定服务
service = store.for_store().find_service("weather")
health = service.check_health()
if health['status'] == 'healthy':
    print("✅ weather 服务健康")
```

### 场景 3：深度诊断
```python
# 详细诊断服务问题
service = store.for_store().find_service("weather")
details = service.health_details()

if details['errors']:
    print(f"发现 {len(details['errors'])} 个错误:")
    for error in details['errors']:
        print(f"  - {error}")

print(f"工具数量: {details['tools_count']}")
print(f"运行时间: {details['uptime']} 秒")
```

### 场景 4：定期健康巡检
```python
import time

# 定期检查
while True:
    health = store.for_store().check_services()
    print(f"健康服务: {health['healthy']}/{health['total']}")
    
    if health['unhealthy'] > 0:
        print("⚠️ 发现不健康服务，开始详细检查...")
        for svc_name, svc_health in health['services'].items():
            if svc_health['status'] != 'healthy':
                service = store.for_store().find_service(svc_name)
                details = service.health_details()
                print(f"服务 {svc_name} 详情: {details}")
    
    time.sleep(60)  # 每分钟检查一次
```

## 📊 健康状态值

### status 字段可能的值

| 状态 | 含义 | 说明 |
|------|------|------|
| `healthy` | 健康 | 服务正常运行 |
| `unhealthy` | 不健康 | 服务存在问题 |
| `degraded` | 降级 | 部分功能受限 |
| `unknown` | 未知 | 无法确定健康状态 |

### state 字段可能的值

| 状态 | 含义 | 说明 |
|------|------|------|
| `pending` | 等待中 | 服务正在初始化 |
| `connecting` | 连接中 | 正在建立连接 |
| `running` | 运行中 | 服务正常运行 |
| `error` | 错误 | 服务出现错误 |
| `stopped` | 已停止 | 服务已停止 |

## 💡 最佳实践

### 1. 分层健康检查
```python
# 第一层：整体检查
health = store.for_store().check_services()
if health['unhealthy'] > 0:
    # 第二层：单服务检查
    for svc_name in health['services']:
        if health['services'][svc_name]['status'] != 'healthy':
            service = store.for_store().find_service(svc_name)
            # 第三层：详细诊断
            details = service.health_details()
            print(f"服务 {svc_name} 详情: {details}")
```

### 2. 健康检查结果缓存
```python
# 避免频繁检查
import time

health_cache = {}
CACHE_TTL = 30  # 30秒缓存

def get_health_with_cache(store):
    now = time.time()
    if 'timestamp' in health_cache:
        if now - health_cache['timestamp'] < CACHE_TTL:
            return health_cache['data']
    
    health = store.for_store().check_services()
    health_cache['data'] = health
    health_cache['timestamp'] = now
    return health
```

### 3. 健康检查告警
```python
def check_and_alert(store):
    health = store.for_store().check_services()
    
    if health['unhealthy'] > 0:
        # 发送告警
        alert_message = f"⚠️ 发现 {health['unhealthy']} 个不健康服务"
        for svc_name, svc_health in health['services'].items():
            if svc_health['status'] != 'healthy':
                alert_message += f"\n  - {svc_name}: {svc_health['status']}"
        
        # 这里可以集成告警系统
        print(alert_message)
        return False
    return True
```

## 🔗 相关文档

- [check_services() 文档](../../../mcpstore_docs/docs/services/health/check-services.md)
- [check_health() 文档](../../../mcpstore_docs/docs/services/health/check-health.md)
- [health_details() 文档](../../../mcpstore_docs/docs/services/health/health-details.md)
- [健康状态桥梁机制](../../../mcpstore_docs/docs/advanced/health-status-bridge.md)

