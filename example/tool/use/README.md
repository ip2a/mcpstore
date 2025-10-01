# 工具使用测试模块

本模块包含工具调用相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_tool_use_call.py` | Store 调用工具 | Store 级别 |
| `test_store_tool_use_alias.py` | Store 使用工具（别名） | Store 级别 |
| `test_store_tool_use_session.py` | Store 会话模式工具调用 | Store 级别 |
| `test_store_tool_use_session_with.py` | Store With 会话模式 | Store 级别 |
| `test_agent_tool_use_call.py` | Agent 调用工具 | Agent 级别 |
| `test_agent_tool_use_alias.py` | Agent 使用工具（别名） | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 调用工具
python example/tool/use/test_store_tool_use_call.py

# Store 使用工具（别名）
python example/tool/use/test_store_tool_use_alias.py

# Agent 调用工具
python example/tool/use/test_agent_tool_use_call.py

# Agent 使用工具（别名）
python example/tool/use/test_agent_tool_use_alias.py

# Store 会话模式工具调用
python example/tool/use/test_store_tool_use_session.py

# Store With 会话模式
python example/tool/use/test_store_tool_use_session_with.py
```

### 运行所有工具使用测试

```bash
# Windows
for %f in (example\tool\use\test_*.py) do python %f

# Linux/Mac
for f in example/tool/use/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 调用工具
测试 `call_tool()` 方法：
- 直接调用工具
- 参数传递和验证
- 结果处理和展示
- 错误处理
- 性能测试

### 2. Store 使用工具（别名）
测试 `use_tool()` 方法：
- call_tool() 的别名功能
- 功能对比测试
- 性能对比测试
- 使用场景分析

### 3. Agent 调用工具
测试 Agent 上下文中的 `call_tool()`：
- Agent 上下文调用
- 状态隔离测试
- 并发调用测试
- 权限控制测试

### 4. Agent 使用工具（别名）
测试 Agent 上下文中的 `use_tool()`：
- Agent 上下文中的别名功能
- 多 Agent 隔离测试
- 方法对比测试

### 5. Store 会话模式工具调用
测试会话模式下的工具调用：
- 创建和管理会话
- 会话状态持久化
- 多次调用共享状态
- 适用于需要状态保持的场景

### 6. Store With 会话模式
测试 with 上下文管理器：
- 自动资源管理
- 异常安全的会话清理
- Python 惯用法
- 推荐的会话使用方式

## 💡 核心概念

### 两种调用方法

| 方法 | 功能 | 用途 | 示例 |
|------|------|------|------|
| `call_tool()` | 直接调用工具 | 强调调用动作 | `tool.call_tool(params)` |
| `use_tool()` | 使用工具（别名） | 强调使用工具 | `tool.use_tool(params)` |

### 两种上下文

| 上下文 | 特点 | 用途 | 示例 |
|--------|------|------|------|
| Store | 全局共享 | 系统级调用 | `store.for_store().find_tool()` |
| Agent | 独立隔离 | 多 Agent 系统 | `store.for_agent("id").find_tool()` |

## 🎯 使用场景

### 场景 1：直接工具调用
```python
# Store 上下文
tool = store.for_store().find_tool("get_weather")
result = tool.call_tool({"query": "北京"})

# Agent 上下文
agent = store.for_agent("agent_1")
tool = agent.find_tool("get_weather")
result = tool.call_tool({"query": "北京"})
```

### 场景 2：批量工具调用
```python
# 批量调用多个工具
tools = store.for_store().list_tools()
results = []

for tool in tools:
    proxy = store.for_store().find_tool(tool.name)
    try:
        result = proxy.call_tool({"query": "test"})
        results.append(result)
    except Exception as e:
        print(f"工具 {tool.name} 调用失败: {e}")

print(f"成功调用 {len(results)} 个工具")
```

### 场景 3：多 Agent 并发调用
```python
# 创建多个 Agent
agents = []
for i in range(3):
    agent = store.for_agent(f"agent_{i}")
    agents.append(agent)

# 并发调用相同工具
import threading

def call_tool_in_agent(agent_id, agent):
    tool = agent.find_tool("get_weather")
    result = tool.call_tool({"query": f"城市{agent_id}"})
    print(f"Agent {agent_id}: {result}")

# 启动多个线程
threads = []
for i, agent in enumerate(agents):
    thread = threading.Thread(target=call_tool_in_agent, args=(i, agent))
    threads.append(thread)
    thread.start()

# 等待所有线程完成
for thread in threads:
    thread.join()
```

### 场景 4：工具链调用
```python
# 工具链：天气查询 -> 数据分析 -> 报告生成
def tool_chain():
    # 1. 获取天气数据
    weather_tool = store.for_store().find_tool("get_weather")
    weather_data = weather_tool.call_tool({"query": "北京"})
    
    # 2. 分析数据
    analysis_tool = store.for_store().find_tool("analyze_data")
    analysis_result = analysis_tool.call_tool({"data": weather_data})
    
    # 3. 生成报告
    report_tool = store.for_store().find_tool("generate_report")
    report = report_tool.call_tool({
        "data": weather_data,
        "analysis": analysis_result
    })
    
    return report

result = tool_chain()
print(f"工具链执行完成: {result}")
```

## 📊 方法对比

### call_tool() vs use_tool()

| 方面 | call_tool() | use_tool() |
|------|-------------|------------|
| **功能** | 直接调用工具 | call_tool() 的别名 |
| **性能** | 相同 | 相同 |
| **语义** | 强调"调用" | 强调"使用" |
| **推荐** | 系统级调用 | 用户级使用 |

### Store vs Agent 上下文

| 方面 | Store 上下文 | Agent 上下文 |
|------|-------------|--------------|
| **状态** | 全局共享 | 独立隔离 |
| **并发** | 共享状态 | 支持并发 |
| **权限** | 系统级 | 可配置 |
| **用途** | 系统调用 | 多 Agent 系统 |

## 💡 最佳实践

### 1. 参数验证
```python
def safe_call_tool(tool_name, params):
    """安全的工具调用"""
    try:
        tool = store.for_store().find_tool(tool_name)
        schema = tool.tool_schema()
        
        # 验证必填参数
        if 'required' in schema:
            for field in schema['required']:
                if field not in params:
                    raise ValueError(f"缺少必填参数: {field}")
        
        # 调用工具
        result = tool.call_tool(params)
        return result
        
    except Exception as e:
        print(f"工具调用失败: {e}")
        return None
```

### 2. 错误处理
```python
def robust_tool_call(tool_name, params, max_retries=3):
    """健壮的工具调用"""
    for attempt in range(max_retries):
        try:
            tool = store.for_store().find_tool(tool_name)
            result = tool.call_tool(params)
            return result
            
        except Exception as e:
            print(f"尝试 {attempt + 1} 失败: {e}")
            if attempt == max_retries - 1:
                raise e
            time.sleep(1)  # 等待重试
```

### 3. 结果处理
```python
def process_tool_result(result):
    """处理工具调用结果"""
    if isinstance(result, dict):
        # 提取关键信息
        if 'content' in result:
            return result['content']
        elif 'data' in result:
            return result['data']
        else:
            return result
    else:
        return str(result)
```

### 4. 性能优化
```python
def batch_tool_calls(tool_requests):
    """批量工具调用"""
    results = []
    
    for tool_name, params in tool_requests:
        try:
            tool = store.for_store().find_tool(tool_name)
            result = tool.call_tool(params)
            results.append({
                'tool': tool_name,
                'success': True,
                'result': result
            })
        except Exception as e:
            results.append({
                'tool': tool_name,
                'success': False,
                'error': str(e)
            })
    
    return results
```

## 🔧 常见问题

### Q1: call_tool() 和 use_tool() 有什么区别？
**A**: 没有功能区别，`use_tool()` 是 `call_tool()` 的别名，提供更语义化的方法名。

### Q2: Store 和 Agent 上下文调用结果相同吗？
**A**: 通常相同，但 Agent 上下文支持状态隔离和权限控制，可能在某些情况下有差异。

### Q3: 如何处理工具调用失败？
**A**: 
```python
try:
    result = tool.call_tool(params)
except Exception as e:
    print(f"调用失败: {e}")
    # 处理错误
```

### Q4: 可以并发调用工具吗？
**A**: 可以，特别是在 Agent 上下文中，每个 Agent 有独立的状态。

### Q5: 如何优化工具调用性能？
**A**: 
- 缓存工具代理对象
- 批量调用
- 异步调用
- 结果缓存

## 🔗 相关文档

- [call_tool() 文档](../../../mcpstore_docs/docs/tools/usage/call-tool.md)
- [use_tool() 文档](../../../mcpstore_docs/docs/tools/usage/use-tool.md)
- [ToolProxy 文档](../../../mcpstore_docs/docs/tools/finding/tool-proxy.md)
- [Agent 上下文文档](../../../mcpstore_docs/docs/advanced/concepts.md)

