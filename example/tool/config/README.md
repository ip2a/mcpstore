# 工具配置测试模块

本模块包含工具配置相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_tool_config_redirect.py` | Store 设置工具重定向 | Store 级别 |
| `test_agent_tool_config_redirect.py` | Agent 设置工具重定向 | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 设置工具重定向
python example/tool/config/test_store_tool_config_redirect.py

# Agent 设置工具重定向
python example/tool/config/test_agent_tool_config_redirect.py
```

### 运行所有工具配置测试

```bash
# Windows
for %f in (example\tool\config\test_*.py) do python %f

# Linux/Mac
for f in example/tool/config/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 设置工具重定向
测试 `set_redirect()` 方法：
- 设置工具重定向行为
- 重定向状态切换
- 重定向行为测试
- 多工具重定向设置

### 2. Agent 设置工具重定向
测试 Agent 上下文中的 `set_redirect()`：
- Agent 上下文重定向设置
- 状态隔离测试
- 多 Agent 重定向隔离
- 并发配置测试

## 💡 核心概念

### 重定向功能

| 方法 | 功能 | 用途 | 示例 |
|------|------|------|------|
| `set_redirect(True)` | 启用重定向 | 直接返回结果 | LangChain return_direct |
| `set_redirect(False)` | 禁用重定向 | 正常处理结果 | 标准工具调用 |
| `set_redirect()` | 获取状态 | 查看当前设置 | 状态检查 |

### 重定向行为

| 设置 | 行为 | 用途 | 影响 |
|------|------|------|------|
| `True` | 直接返回 | 跳过中间处理 | 性能优化 |
| `False` | 正常处理 | 标准流程 | 完整处理 |

## 🎯 使用场景

### 场景 1：LangChain 集成
```python
# 设置工具重定向以支持 LangChain return_direct
tool = store.for_store().find_tool("get_weather")

# 启用重定向
tool.set_redirect(True)

# 在 LangChain 中使用
from langchain.tools import Tool
langchain_tool = Tool(
    name="weather",
    func=lambda query: tool.call_tool({"query": query}),
    return_direct=True  # 对应 set_redirect(True)
)
```

### 场景 2：工具链优化
```python
# 优化工具链性能
def optimized_tool_chain():
    # 设置关键工具重定向
    weather_tool = store.for_store().find_tool("get_weather")
    weather_tool.set_redirect(True)  # 直接返回天气数据
    
    # 调用工具
    weather_data = weather_tool.call_tool({"query": "北京"})
    
    # 处理数据
    processed_data = process_weather_data(weather_data)
    
    return processed_data
```

### 场景 3：多 Agent 重定向配置
```python
# 不同 Agent 使用不同的重定向策略
def setup_agent_redirects():
    # Agent 1: 启用重定向（快速响应）
    agent1 = store.for_agent("fast_agent")
    tool1 = agent1.find_tool("get_weather")
    tool1.set_redirect(True)
    
    # Agent 2: 禁用重定向（完整处理）
    agent2 = store.for_agent("thorough_agent")
    tool2 = agent2.find_tool("get_weather")
    tool2.set_redirect(False)
    
    return agent1, agent2
```

### 场景 4：动态重定向控制
```python
# 根据条件动态设置重定向
def dynamic_redirect_control(tool_name, use_redirect):
    tool = store.for_store().find_tool(tool_name)
    
    # 设置重定向
    tool.set_redirect(use_redirect)
    
    # 验证设置
    current_status = tool.set_redirect()
    print(f"工具 {tool_name} 重定向状态: {current_status}")
    
    return tool
```

## 📊 重定向对比

### 重定向 vs 非重定向

| 方面 | 重定向=True | 重定向=False |
|------|-------------|--------------|
| **性能** | 更快 | 标准 |
| **处理** | 跳过中间步骤 | 完整处理 |
| **结果** | 直接返回 | 处理后返回 |
| **用途** | 框架集成 | 标准调用 |

### Store vs Agent 重定向

| 方面 | Store 上下文 | Agent 上下文 |
|------|-------------|--------------|
| **作用域** | 全局 | 独立 |
| **隔离** | 共享 | 独立 |
| **并发** | 共享状态 | 支持并发 |
| **权限** | 系统级 | 可配置 |

## 💡 最佳实践

### 1. 重定向状态管理
```python
class ToolRedirectManager:
    """工具重定向管理器"""
    
    def __init__(self, store):
        self.store = store
        self.redirect_states = {}
    
    def set_tool_redirect(self, tool_name, redirect=True):
        """设置工具重定向"""
        tool = self.store.for_store().find_tool(tool_name)
        tool.set_redirect(redirect)
        self.redirect_states[tool_name] = redirect
        return tool
    
    def get_tool_redirect(self, tool_name):
        """获取工具重定向状态"""
        tool = self.store.for_store().find_tool(tool_name)
        return tool.set_redirect()
    
    def reset_all_redirects(self):
        """重置所有工具重定向"""
        for tool_name in self.redirect_states:
            tool = self.store.for_store().find_tool(tool_name)
            tool.set_redirect(False)
        self.redirect_states.clear()
```

### 2. 条件重定向
```python
def conditional_redirect(tool_name, condition):
    """条件重定向"""
    tool = store.for_store().find_tool(tool_name)
    
    if condition:
        tool.set_redirect(True)
        print(f"工具 {tool_name} 启用重定向")
    else:
        tool.set_redirect(False)
        print(f"工具 {tool_name} 禁用重定向")
    
    return tool
```

### 3. 批量重定向设置
```python
def batch_set_redirects(tool_configs):
    """批量设置工具重定向"""
    results = []
    
    for tool_name, redirect in tool_configs:
        try:
            tool = store.for_store().find_tool(tool_name)
            tool.set_redirect(redirect)
            current_status = tool.set_redirect()
            
            results.append({
                'tool': tool_name,
                'requested': redirect,
                'actual': current_status,
                'success': True
            })
        except Exception as e:
            results.append({
                'tool': tool_name,
                'requested': redirect,
                'error': str(e),
                'success': False
            })
    
    return results
```

### 4. 重定向状态监控
```python
def monitor_redirect_states():
    """监控重定向状态"""
    tools = store.for_store().list_tools()
    redirect_report = {}
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        redirect_status = proxy.set_redirect()
        redirect_report[tool.name] = redirect_status
    
    return redirect_report
```

## 🔧 常见问题

### Q1: 重定向是什么？
**A**: 重定向是工具的一种行为模式，启用后工具会直接返回结果，跳过中间处理步骤。

### Q2: 什么时候使用重定向？
**A**: 
- LangChain 集成时
- 需要直接返回结果时
- 性能优化时
- 框架适配时

### Q3: 重定向影响结果内容吗？
**A**: 通常不影响结果内容，主要影响处理流程和性能。

### Q4: 如何检查重定向状态？
**A**: 
```python
tool = store.for_store().find_tool("tool_name")
status = tool.set_redirect()  # 不传参数获取状态
print(f"重定向状态: {status}")
```

### Q5: 重定向设置是永久的吗？
**A**: 不是，可以随时通过 `set_redirect()` 方法修改。

## 🔗 相关文档

- [set_redirect() 文档](../../../mcpstore_docs/docs/tools/config/set-redirect.md)
- [ToolProxy 文档](../../../mcpstore_docs/docs/tools/finding/tool-proxy.md)
- [LangChain 集成文档](../../../mcpstore_docs/docs/tools/langchain/langchain-list-tools.md)
- [Agent 上下文文档](../../../mcpstore_docs/docs/advanced/concepts.md)
