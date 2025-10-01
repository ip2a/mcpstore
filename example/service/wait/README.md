# 等待服务测试模块

本模块包含服务等待相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_wait_basic.py` | Store 等待服务就绪（基础） | Store 级别 |
| `test_store_service_wait_timeout.py` | Store 等待服务超时 | Store 级别 |
| `test_agent_service_wait_basic.py` | Agent 等待服务就绪（基础） | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 等待服务（基础）
python example/service/wait/test_store_service_wait_basic.py

# Store 等待服务超时
python example/service/wait/test_store_service_wait_timeout.py

# Agent 等待服务
python example/service/wait/test_agent_service_wait_basic.py
```

### 运行所有等待服务测试

```bash
# Windows
for %f in (example\service\wait\test_*.py) do python %f

# Linux/Mac
for f in example/service/wait/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 等待服务（基础）
测试 `wait_service()` 基础功能：
- 添加服务后等待就绪
- 记录等待时间
- 验证服务可用（列出工具）
- 再次等待已就绪的服务（立即返回）

### 2. Store 等待服务超时
测试超时机制：
- 合理的超时时间
- 等待不存在的服务（异常）
- 不同的超时时间测试
- 批量等待多个服务

### 3. Agent 等待服务（基础）
测试 Agent 级别的等待：
- Agent 等待自己的服务
- 创建多个 Agent 独立等待
- 验证服务独立性
- Store 无法等待 Agent 服务

## 💡 核心概念

### wait_service() 方法

```python
# 基本用法
result = store.for_store().wait_service(
    service_name="weather",
    timeout=30.0  # 超时时间（秒）
)

# Agent 级别
result = agent.wait_service("weather", timeout=30.0)
```

### 方法签名

```python
def wait_service(
    service_name: str,
    timeout: float = 30.0
) -> bool:
    """
    等待服务达到就绪状态
    
    参数:
        service_name: 服务名称
        timeout: 超时时间（秒），默认 30.0
    
    返回:
        bool: 服务是否就绪
    
    异常:
        TimeoutError: 超时
        ServiceNotFoundError: 服务不存在
    """
```

## 🎯 使用场景

### 场景 1：添加服务后确保可用
```python
# 添加服务
store.for_store().add_service({
    "mcpServers": {
        "weather": {"url": "https://..."}
    }
})

# 等待就绪
store.for_store().wait_service("weather", timeout=30.0)

# 现在可以安全使用
tools = store.for_store().list_tools()
```

### 场景 2：批量添加服务后等待
```python
# 批量添加
store.for_store().add_service({
    "mcpServers": {
        "service1": {"url": "https://..."},
        "service2": {"url": "https://..."},
        "service3": {"url": "https://..."}
    }
})

# 逐个等待
services = ["service1", "service2", "service3"]
for svc in services:
    store.for_store().wait_service(svc, timeout=30.0)
    print(f"✅ {svc} 就绪")
```

### 场景 3：服务重启后等待恢复
```python
# 重启服务
service = store.for_store().find_service("weather")
service.restart_service()

# 等待恢复
store.for_store().wait_service("weather", timeout=30.0)
print("服务已恢复")
```

### 场景 4：Agent 独立等待
```python
# Agent1 等待
agent1 = store.for_agent("user1")
agent1.add_service({...})
agent1.wait_service("weather", timeout=30.0)

# Agent2 等待（独立）
agent2 = store.for_agent("user2")
agent2.add_service({...})
agent2.wait_service("search", timeout=30.0)
```

## 📊 超时时间建议

| 服务类型 | 建议超时 | 说明 |
|---------|---------|------|
| **本地服务** | 10-15秒 | 本地启动较快 |
| **远程服务（国内）** | 20-30秒 | 网络延迟 |
| **远程服务（国外）** | 30-60秒 | 更长的网络延迟 |
| **复杂服务** | 60秒+ | 需要初始化时间 |
| **开发测试** | 5-10秒 | 快速失败 |
| **生产环境** | 30-60秒 | 容忍网络波动 |

## 🔧 错误处理

### 超时处理
```python
try:
    store.for_store().wait_service("weather", timeout=10.0)
except TimeoutError as e:
    print(f"服务等待超时: {e}")
    # 处理超时情况
except Exception as e:
    print(f"等待失败: {e}")
```

### 服务不存在
```python
try:
    store.for_store().wait_service("nonexistent", timeout=5.0)
except Exception as e:
    print(f"服务不存在或等待失败: {e}")
    # 检查服务是否已添加
    services = store.for_store().list_services()
    print(f"可用服务: {[s.name for s in services]}")
```

## 💡 最佳实践

### 1. 添加服务后立即等待
```python
# ✅ 推荐
store.for_store().add_service({...})
store.for_store().wait_service("weather")  # 确保就绪
result = store.for_store().use_tool("get_weather", {...})

# ❌ 不推荐（可能服务未就绪）
store.for_store().add_service({...})
result = store.for_store().use_tool("get_weather", {...})  # 可能失败
```

### 2. 设置合理的超时
```python
# ✅ 根据服务类型设置
# 本地服务
store.for_store().wait_service("local_service", timeout=10.0)

# 远程服务
store.for_store().wait_service("remote_service", timeout=30.0)
```

### 3. 批量等待时记录时间
```python
import time

services = ["s1", "s2", "s3"]
for svc in services:
    start = time.time()
    store.for_store().wait_service(svc, timeout=30.0)
    elapsed = time.time() - start
    print(f"{svc} 就绪，耗时: {elapsed:.2f}s")
```

### 4. 生产环境增加重试
```python
def wait_with_retry(store, service_name, max_retries=3):
    for i in range(max_retries):
        try:
            store.for_store().wait_service(service_name, timeout=30.0)
            return True
        except Exception as e:
            if i == max_retries - 1:
                raise
            print(f"重试 {i+1}/{max_retries}...")
            time.sleep(5)
    return False
```

## 📈 性能考虑

### 并行等待（多 Agent）
```python
import concurrent.futures

def wait_agent_service(agent_id, service_name):
    agent = store.for_agent(agent_id)
    agent.add_service({...})
    agent.wait_service(service_name, timeout=30.0)
    return f"Agent {agent_id} 就绪"

# 并行等待多个 Agent
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
    futures = [
        executor.submit(wait_agent_service, f"agent{i}", "weather")
        for i in range(5)
    ]
    for future in concurrent.futures.as_completed(futures):
        print(future.result())
```

## 🔗 相关文档

- [wait_service() 文档](../../../mcpstore_docs/docs/services/waiting/wait-service.md)
- [服务生命周期](../../../mcpstore_docs/docs/advanced/lifecycle.md)
- [添加服务文档](../../../mcpstore_docs/docs/services/registration/add-service.md)

