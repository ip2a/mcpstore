# 服务详情测试模块

本模块包含服务详情查询相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_detail_info.py` | Store 获取服务信息 | Store 级别 |
| `test_store_service_detail_status.py` | Store 获取服务状态 | Store 级别 |
| `test_agent_service_detail_info.py` | Agent 获取服务信息 | Agent 级别 |
| `test_agent_service_detail_status.py` | Agent 获取服务状态 | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 获取服务信息
python example/service/detail/test_store_service_detail_info.py

# Store 获取服务状态
python example/service/detail/test_store_service_detail_status.py

# Agent 获取服务信息
python example/service/detail/test_agent_service_detail_info.py

# Agent 获取服务状态
python example/service/detail/test_agent_service_detail_status.py
```

### 运行所有服务详情测试

```bash
# Windows
for %f in (example\service\detail\test_*.py) do python %f

# Linux/Mac
for f in example/service/detail/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 获取服务信息
测试 `service_info()` 方法：
- 获取服务的详细配置信息
- 展示服务名称、类型、配置
- 查看完整的 JSON 格式信息
- 对比不同类型服务的信息差异

### 2. Store 获取服务状态
测试 `service_status()` 方法：
- 获取服务的实时运行状态
- 查看生命周期状态（state）
- 查看健康状态（health）
- 对比添加前后的状态变化
- 区分 info 和 status 的不同

### 3. Agent 获取服务信息
测试 Agent 级别的 `service_info()`：
- Agent 查询自己的服务信息
- 对比多个 Agent 的服务信息
- 验证信息隔离性

### 4. Agent 获取服务状态
测试 Agent 级别的 `service_status()`：
- Agent 查询自己的服务状态
- 对比多个 Agent 的服务状态
- 验证状态隔离性

## 💡 核心概念

### service_info() vs service_status()

| 方法 | 用途 | 数据类型 | 更新频率 | 主要字段 |
|------|------|----------|----------|----------|
| **service_info()** | 服务配置信息 | 静态 | 配置变更时 | name, type, config |
| **service_status()** | 服务运行状态 | 动态 | 实时更新 | state, health, connected |

### service_info() 返回字段

```python
info = service_proxy.service_info()

# 常见字段
{
    "name": "weather",              # 服务名称
    "type": "url",                  # 服务类型（url/command/market）
    "config": {                     # 服务配置
        "url": "https://..."
    },
    "created_at": "2025-01-09...",  # 创建时间
    "updated_at": "2025-01-09..."   # 更新时间
}
```

### service_status() 返回字段

```python
status = service_proxy.service_status()

# 常见字段
{
    "state": "running",             # 生命周期状态
    "health": "healthy",            # 健康状态
    "connected": true,              # 连接状态
    "last_check": "2025-01-09...",  # 最后检查时间
    "uptime": 3600,                 # 运行时长（秒）
    "errors": []                    # 错误列表
}
```

## 🎯 使用场景

### 场景 1：查看服务配置
```python
# 查看服务的完整配置
service = store.for_store().find_service("weather")
info = service.service_info()
print(f"服务类型: {info['type']}")
print(f"配置: {info['config']}")
```

### 场景 2：监控服务状态
```python
# 实时监控服务运行状态
service = store.for_store().find_service("weather")
status = service.service_status()
print(f"状态: {status['state']}")
print(f"健康: {status['health']}")
```

### 场景 3：调试服务问题
```python
# 同时查看配置和状态
service = store.for_store().find_service("weather")
info = service.service_info()
status = service.service_status()

print(f"配置: {info['config']}")
print(f"状态: {status['state']}")
print(f"健康: {status['health']}")
```

### 场景 4：Agent 隔离查询
```python
# 每个 Agent 查询自己的服务
agent1 = store.for_agent("user1")
service1 = agent1.find_service("weather")
info1 = service1.service_info()

agent2 = store.for_agent("user2")
service2 = agent2.find_service("search")
info2 = service2.service_info()

# 完全隔离
```

## 📊 字段对比

### 信息字段（service_info）

| 字段 | 类型 | 说明 | 示例 |
|------|------|------|------|
| `name` | string | 服务名称 | "weather" |
| `type` | string | 服务类型 | "url" / "command" / "market" |
| `config` | object | 服务配置 | `{"url": "..."}` |
| `created_at` | string | 创建时间 | ISO 8601 格式 |
| `updated_at` | string | 更新时间 | ISO 8601 格式 |

### 状态字段（service_status）

| 字段 | 类型 | 说明 | 可能值 |
|------|------|------|--------|
| `state` | string | 生命周期状态 | "pending" / "connecting" / "running" / "error" |
| `health` | string | 健康状态 | "healthy" / "unhealthy" / "unknown" |
| `connected` | boolean | 连接状态 | true / false |
| `last_check` | string | 最后检查时间 | ISO 8601 格式 |
| `uptime` | number | 运行时长（秒） | 3600 |
| `errors` | array | 错误列表 | [] |

## 💡 最佳实践

### 1. 配置调试时使用 service_info()
```python
# 调试配置问题
info = service.service_info()
print(json.dumps(info, indent=2))
```

### 2. 状态监控时使用 service_status()
```python
# 实时监控
status = service.service_status()
if status['health'] != 'healthy':
    print("服务不健康！")
```

### 3. 完整诊断时结合使用
```python
# 完整诊断
info = service.service_info()
status = service.service_status()
print(f"配置类型: {info['type']}")
print(f"运行状态: {status['state']}")
print(f"健康状态: {status['health']}")
```

## 🔗 相关文档

- [service_info() 文档](../../../mcpstore_docs/docs/services/details/service-info.md)
- [service_status() 文档](../../../mcpstore_docs/docs/services/details/service-status.md)
- [ServiceProxy 文档](../../../mcpstore_docs/docs/services/listing/service-proxy.md)

