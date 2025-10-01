# 查找工具测试模块

本模块包含工具查找和列举相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_tool_find_basic.py` | Store 查找工具（基础） | Store 级别 |
| `test_store_tool_find_list.py` | Store 列出所有工具 | Store 级别 |
| `test_agent_tool_find_basic.py` | Agent 查找工具（基础） | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 查找工具
python example/tool/find/test_store_tool_find_basic.py

# Store 列出所有工具
python example/tool/find/test_store_tool_find_list.py

# Agent 查找工具
python example/tool/find/test_agent_tool_find_basic.py
```

### 运行所有查找工具测试

```bash
# Windows
for %f in (example\tool\find\test_*.py) do python %f

# Linux/Mac
for f in example/tool/find/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 查找工具（基础）
测试 `find_tool()` 方法：
- 查找特定工具
- 返回 ToolProxy 对象
- 验证 ToolProxy 方法
- 使用 ToolProxy 获取信息和调用工具
- 查找不存在的工具

### 2. Store 列出所有工具
测试 `list_tools()` 方法：
- 列出所有可用工具
- 返回 ToolInfo 对象列表
- 遍历工具列表
- 按服务分组工具
- 工具统计分析

### 3. Agent 查找工具（基础）
测试 Agent 级别的工具查找：
- Agent 查找自己的工具
- 验证工具隔离性
- 对比多个 Agent 的工具
- Store 无法看到 Agent 工具

## 💡 核心概念

### find_tool() vs list_tools()

| 方法 | 返回类型 | 用途 | 适用场景 |
|------|----------|------|----------|
| `find_tool(name)` | ToolProxy | 获取工具操作代理 | 单个工具操作 |
| `list_tools()` | List[ToolInfo] | 获取工具列表 | 批量查询、遍历 |

### ToolProxy vs ToolInfo

| 类型 | 获取方式 | 用途 | 可用方法 |
|------|----------|------|----------|
| **ToolProxy** | `find_tool(name)` | 工具操作代理 | 完整的工具管理方法 |
| **ToolInfo** | `list_tools()` 返回 | 工具基本信息 | 只读属性（name, description等）|

### ToolProxy 主要方法

```python
tool_proxy = store.for_store().find_tool("tool_name")

# 信息查询
tool_proxy.tool_info()          # 获取工具详细信息
tool_proxy.tool_tags()          # 获取工具标签
tool_proxy.tool_schema()        # 获取工具输入模式

# 工具调用
tool_proxy.call_tool({...})     # 调用工具

# 工具配置
tool_proxy.set_redirect(True)   # 设置重定向标记

# 统计信息
tool_proxy.usage_stats()        # 获取使用统计
tool_proxy.call_history()       # 获取调用历史
```

## 🎯 使用场景

### 场景 1：查找单个工具并调用
```python
# 查找工具
tool = store.for_store().find_tool("get_weather")

# 获取信息
info = tool.tool_info()
print(f"工具描述: {info['description']}")

# 调用工具
result = tool.call_tool({"query": "北京"})
print(f"结果: {result}")
```

### 场景 2：遍历所有工具
```python
# 列出所有工具
tools = store.for_store().list_tools()

# 批量操作
for tool in tools:
    print(f"工具: {tool.name}")
    # 需要详细操作时获取 ToolProxy
    proxy = store.for_store().find_tool(tool.name)
    stats = proxy.usage_stats()
    print(f"调用次数: {stats.get('count', 0)}")
```

### 场景 3：Agent 隔离工具
```python
# Agent1 的工具
agent1 = store.for_agent("user1")
agent1.add_service({...})
agent1_tools = agent1.list_tools()

# Agent2 的工具
agent2 = store.for_agent("user2")
agent2.add_service({...})
agent2_tools = agent2.list_tools()

# 完全隔离
```

### 场景 4：按服务查找工具
```python
# 查找特定服务的工具
service = store.for_store().find_service("weather")
service_tools = service.list_tools()
print(f"weather 服务的工具: {[t.name for t in service_tools]}")
```

## 📊 方法对比

| 方法 | 级别 | 返回类型 | 用途 | 示例 |
|------|------|----------|------|------|
| `find_tool(name)` | Context | ToolProxy | 查找单个工具 | `store.for_store().find_tool("get_weather")` |
| `list_tools()` | Context | List[ToolInfo] | 列出所有工具 | `store.for_store().list_tools()` |
| `list_tools()` | ServiceProxy | List[ToolInfo] | 列出服务工具 | `service_proxy.list_tools()` |

## 💡 最佳实践

### 1. 优先使用 list_tools() 发现工具
```python
# ✅ 推荐：先列出，再查找
tools = store.for_store().list_tools()
if any(t.name == "get_weather" for t in tools):
    tool = store.for_store().find_tool("get_weather")
    result = tool.call_tool({...})
```

### 2. 缓存 ToolProxy
```python
# 如果需要多次操作同一个工具
tool_cache = {}

def get_tool(tool_name):
    if tool_name not in tool_cache:
        tool_cache[tool_name] = store.for_store().find_tool(tool_name)
    return tool_cache[tool_name]

# 多次使用
tool = get_tool("get_weather")
tool.call_tool({...})
tool.usage_stats()
```

### 3. 按服务分组工具
```python
# 按服务查看工具分布
services = store.for_store().list_services()
for service in services:
    proxy = store.for_store().find_service(service.name)
    tools = proxy.list_tools()
    print(f"{service.name}: {len(tools)} 个工具")
```

### 4. 工具名称搜索
```python
def search_tools(keyword):
    """搜索工具名称"""
    tools = store.for_store().list_tools()
    results = [t for t in tools if keyword.lower() in t.name.lower()]
    return results

# 搜索包含 "weather" 的工具
weather_tools = search_tools("weather")
```

## 🔧 常见问题

### Q1: find_tool() 和 list_tools() 的区别？
**A**: 
- `find_tool()`: 查找单个工具，返回 ToolProxy，用于操作
- `list_tools()`: 列出所有工具，返回 ToolInfo 列表，用于浏览

### Q2: ToolProxy 和 ToolInfo 有什么区别？
**A**: 
- ToolProxy: 操作代理，有完整方法（调用、配置、统计）
- ToolInfo: 信息对象，只有只读属性（name, description）

### Q3: 如何知道工具属于哪个服务？
**A**: 
```python
# 方法1：通过服务查询
service = store.for_store().find_service("weather")
tools = service.list_tools()

# 方法2：工具名称通常包含服务前缀
# 如: mcp_howtocook_getAllRecipes
```

### Q4: Agent 能找到 Store 的工具吗？
**A**: 不能。Agent 和 Store 的工具完全隔离。

### Q5: 工具列表会自动更新吗？
**A**: 不会自动更新。如需更新：
```python
# 刷新服务内容
service = store.for_store().find_service("weather")
service.refresh_content()

# 重新列出工具
tools = store.for_store().list_tools()
```

## 🔗 相关文档

- [find_tool() 文档](../../../mcpstore_docs/docs/tools/finding/find-tool.md)
- [list_tools() 文档](../../../mcpstore_docs/docs/tools/finding/list-tools.md)
- [ToolProxy 概念](../../../mcpstore_docs/docs/tools/finding/tool-proxy.md)
- [工具管理概览](../../../mcpstore_docs/docs/tools/overview.md)

