# 查找服务测试模块

本模块包含服务查找和列举相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_find_basic.py` | Store 查找服务（基础） | Store 级别 |
| `test_store_service_find_list.py` | Store 列出所有服务 | Store 级别 |
| `test_agent_service_find_basic.py` | Agent 查找服务（基础） | Agent 级别 |
| `test_agent_service_find_list.py` | Agent 列出所有服务 | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 查找服务
python example/service/find/test_store_service_find_basic.py

# Store 列出所有服务
python example/service/find/test_store_service_find_list.py

# Agent 查找服务
python example/service/find/test_agent_service_find_basic.py

# Agent 列出所有服务
python example/service/find/test_agent_service_find_list.py
```

### 运行所有查找服务测试

```bash
# Windows
for %f in (example\service\find\test_*.py) do python %f

# Linux/Mac
for f in example/service/find/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 查找服务（基础）
测试 `find_service()` 方法：
- 查找单个服务
- 返回 ServiceProxy 对象
- 验证 ServiceProxy 的方法
- 使用 ServiceProxy 获取服务信息
- 使用 ServiceProxy 获取服务状态
- 使用 ServiceProxy 列出工具

### 2. Store 列出所有服务
测试 `list_services()` 方法：
- 列出所有已注册的服务
- 返回 ServiceInfo 对象列表
- 遍历服务列表
- 从列表中查找特定服务
- 批量等待服务就绪
- 获取每个服务的工具数量

### 3. Agent 查找服务（基础）
测试 Agent 级别的 `find_service()`：
- Agent 查找自己的服务
- 验证 Store 级别找不到 Agent 服务
- 验证不同 Agent 之间的隔离性
- 对比多个 Agent 的服务

### 4. Agent 列出所有服务
测试 Agent 级别的 `list_services()`：
- Agent 列出自己的服务
- 批量操作 Agent 的服务
- 对比不同 Agent 的服务列表
- 验证与 Store 级别的隔离

## 💡 核心概念

### ServiceProxy vs ServiceInfo

| 类型 | 获取方式 | 用途 | 可用方法 |
|------|----------|------|----------|
| **ServiceProxy** | `find_service(name)` | 服务操作代理 | 完整的服务管理方法 |
| **ServiceInfo** | `list_services()` 返回 | 服务基本信息 | 只读属性（name, config 等）|

### ServiceProxy 主要方法

```python
service_proxy = store.for_store().find_service("service_name")

# 信息查询
service_proxy.service_info()        # 获取服务详细信息
service_proxy.service_status()      # 获取服务运行状态

# 健康检查
service_proxy.check_health()        # 获取健康摘要
service_proxy.health_details()      # 获取详细健康信息

# 配置管理
service_proxy.update_config({...})  # 完整更新配置
service_proxy.patch_config({...})   # 增量更新配置

# 生命周期管理
service_proxy.restart_service()     # 重启服务
service_proxy.refresh_content()     # 刷新服务内容
service_proxy.remove_service()      # 移除服务（运行态）
service_proxy.delete_service()      # 完全删除服务

# 工具相关
service_proxy.list_tools()          # 列出服务的工具
service_proxy.tools_stats()         # 获取工具统计
```

## 🎯 使用场景

### 场景 1：查找单个服务并操作
```python
# 查找服务
service = store.for_store().find_service("weather")

# 获取信息
info = service.service_info()
status = service.service_status()

# 列出工具
tools = service.list_tools()
```

### 场景 2：遍历所有服务
```python
# 列出所有服务
services = store.for_store().list_services()

# 批量操作
for svc in services:
    print(f"服务: {svc.name}")
    # 需要更多操作时获取 ServiceProxy
    proxy = store.for_store().find_service(svc.name)
    tools = proxy.list_tools()
    print(f"工具数量: {len(tools)}")
```

### 场景 3：Agent 隔离
```python
# Agent1 的服务
agent1 = store.for_agent("user1")
agent1.add_service({...})
agent1_services = agent1.list_services()  # 只看到自己的

# Agent2 的服务
agent2 = store.for_agent("user2")
agent2.add_service({...})
agent2_services = agent2.list_services()  # 只看到自己的

# 完全隔离
```

## 📊 方法对比

| 方法 | 返回类型 | 用途 | 适用场景 |
|------|----------|------|----------|
| `find_service(name)` | ServiceProxy | 获取服务操作代理 | 单个服务操作 |
| `list_services()` | List[ServiceInfo] | 获取服务列表 | 批量查询、遍历 |

## 🔗 相关文档

- [查找服务文档](../../../mcpstore_docs/docs/services/listing/find-service.md)
- [list_services() 文档](../../../mcpstore_docs/docs/services/listing/list-services.md)
- [ServiceProxy 文档](../../../mcpstore_docs/docs/services/listing/service-proxy.md)

