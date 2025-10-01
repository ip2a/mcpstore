# LangChain 集成测试模块

本模块包含 LangChain 集成相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_langchain_list_tools.py` | LangChain 列出工具 | Store 级别 |
| `test_store_langchain_tool_call.py` | LangChain 工具调用 | Store 级别 |
| `test_store_langchain_tool_chain.py` | LangChain 工具链构建 | Store 级别 |
| `test_store_langchain_agent_basic.py` | LangChain Agent 基础调用 | Store 级别 |
| `test_store_langchain_agent_session.py` | LangChain Agent 会话模式 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# LangChain 列出工具
python example/integration/langchain/test_store_langchain_list_tools.py

# LangChain 工具调用
python example/integration/langchain/test_store_langchain_tool_call.py

# LangChain 工具链构建
python example/integration/langchain/test_store_langchain_tool_chain.py

# LangChain Agent 基础调用
python example/integration/langchain/test_store_langchain_agent_basic.py

# LangChain Agent 会话模式
python example/integration/langchain/test_store_langchain_agent_session.py
```

### 运行所有 LangChain 集成测试

```bash
# Windows
for %f in (example\integration\langchain\test_*.py) do python %f

# Linux/Mac
for f in example/integration/langchain/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. LangChain 列出工具
测试 `for_langchain().list_tools()` 方法：
- 获取 LangChain 兼容的工具列表
- 工具格式转换
- 工具属性分析
- 工具调用测试

### 2. LangChain 工具调用
测试 LangChain 工具的实际调用：
- 工具调用测试
- 参数验证
- 性能测试
- 错误处理

### 3. LangChain 工具链构建
测试使用 LangChain 工具构建工具链：
- 简单工具链
- 复杂工具链
- 条件工具链
- 循环工具链

### 4. LangChain Agent 基础调用
测试 LangChain Agent 使用 MCPStore 工具：
- Agent 创建和配置
- 工具自动选择
- 多步骤任务执行
- 自然语言交互

### 5. LangChain Agent 会话模式
测试 Agent 在会话上下文中使用：
- 会话状态持久化
- With 上下文管理
- 浏览器状态保持
- 适合多步骤复杂任务

## 💡 核心概念

### LangChain 集成

| 方法 | 功能 | 用途 | 示例 |
|------|------|------|------|
| `for_langchain()` | 获取集成对象 | 创建 LangChain 集成 | `store.for_langchain()` |
| `list_tools()` | 列出工具 | 获取工具列表 | `integration.list_tools()` |

### 工具链类型

| 类型 | 特点 | 用途 | 示例 |
|------|------|------|------|
| **简单工具链** | 线性执行 | 基础流程 | 工具1 -> 工具2 |
| **复杂工具链** | 多分支 | 复杂逻辑 | 条件判断 + 工具调用 |
| **条件工具链** | 条件执行 | 动态流程 | if-else + 工具调用 |
| **循环工具链** | 循环执行 | 批量处理 | for + 工具调用 |

## 🎯 使用场景

### 场景 1：基础 LangChain 集成
```python
# 基础 LangChain 集成
def basic_langchain_integration():
    # 获取 LangChain 集成
    langchain_integration = store.for_langchain()
    
    # 获取工具列表
    tools = langchain_integration.list_tools()
    
    # 使用工具
    for tool in tools:
        result = tool.func("测试参数")
        print(f"工具 {tool.name}: {result}")
    
    return tools
```

### 场景 2：工具链构建
```python
# 构建工具链
def build_tool_chain():
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    # 构建简单工具链
    def simple_chain(input_data):
        # 步骤1: 调用工具1
        result1 = tools[0].func(input_data)
        
        # 步骤2: 处理结果
        processed_result = process_result(result1)
        
        # 步骤3: 调用工具2
        result2 = tools[1].func(processed_result)
        
        return result2
    
    return simple_chain
```

### 场景 3：条件工具链
```python
# 条件工具链
def conditional_tool_chain(input_data, condition):
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    if condition == "weather":
        # 天气相关处理
        weather_tool = tools[0]
        result = weather_tool.func(input_data)
        return f"天气信息: {result}"
    
    elif condition == "location":
        # 位置相关处理
        location_tool = tools[1]
        result = location_tool.func(input_data)
        return f"位置信息: {result}"
    
    else:
        # 默认处理
        default_tool = tools[0]
        result = default_tool.func(input_data)
        return f"默认处理: {result}"
```

### 场景 4：循环工具链
```python
# 循环工具链
def loop_tool_chain(inputs):
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    results = []
    for input_data in inputs:
        try:
            # 调用工具
            result = tools[0].func(input_data)
            results.append({
                'input': input_data,
                'result': result,
                'success': True
            })
        except Exception as e:
            results.append({
                'input': input_data,
                'error': str(e),
                'success': False
            })
    
    return results
```

## 📊 集成对比

### 原生工具 vs LangChain 工具

| 方面 | 原生工具 | LangChain 工具 |
|------|----------|----------------|
| **格式** | MCPStore 格式 | LangChain 格式 |
| **接口** | 自定义接口 | 标准 LangChain 接口 |
| **调用** | 直接调用 | 通过 func 调用 |
| **集成** | 原生支持 | 需要转换 |

### 工具链复杂度

| 复杂度 | 特点 | 适用场景 | 示例 |
|--------|------|----------|------|
| **简单** | 线性执行 | 基础流程 | 工具1 -> 工具2 |
| **中等** | 条件分支 | 动态流程 | if-else + 工具 |
| **复杂** | 多分支循环 | 复杂业务 | 嵌套条件 + 循环 |

## 💡 最佳实践

### 1. 工具链设计
```python
class ToolChainBuilder:
    """工具链构建器"""
    
    def __init__(self, store):
        self.store = store
        self.langchain_integration = store.for_langchain()
        self.tools = self.langchain_integration.list_tools()
    
    def build_simple_chain(self, tool_indices):
        """构建简单工具链"""
        def chain(input_data):
            result = input_data
            for index in tool_indices:
                if index < len(self.tools):
                    result = self.tools[index].func(result)
            return result
        return chain
    
    def build_conditional_chain(self, conditions):
        """构建条件工具链"""
        def chain(input_data, condition):
            if condition in conditions:
                tool_index = conditions[condition]
                if tool_index < len(self.tools):
                    return self.tools[tool_index].func(input_data)
            return None
        return chain
```

### 2. 错误处理
```python
def robust_tool_chain(input_data):
    """健壮的工具链"""
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    results = []
    for i, tool in enumerate(tools):
        try:
            result = tool.func(input_data)
            results.append({
                'step': i,
                'tool': tool.name,
                'result': result,
                'success': True
            })
        except Exception as e:
            results.append({
                'step': i,
                'tool': tool.name,
                'error': str(e),
                'success': False
            })
            # 决定是否继续
            if i == 0:  # 第一步失败，停止
                break
    
    return results
```

### 3. 性能优化
```python
def optimized_tool_chain(inputs):
    """优化的工具链"""
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    # 缓存工具
    tool_cache = {}
    for tool in tools:
        tool_cache[tool.name] = tool
    
    # 批量处理
    results = []
    for input_data in inputs:
        # 使用缓存的工具
        tool = tool_cache.get('weather_tool')
        if tool:
            result = tool.func(input_data)
            results.append(result)
    
    return results
```

### 4. 工具链监控
```python
def monitored_tool_chain(input_data):
    """监控的工具链"""
    import time
    
    start_time = time.time()
    
    langchain_integration = store.for_langchain()
    tools = langchain_integration.list_tools()
    
    execution_log = []
    
    for i, tool in enumerate(tools):
        step_start = time.time()
        
        try:
            result = tool.func(input_data)
            step_end = time.time()
            
            execution_log.append({
                'step': i,
                'tool': tool.name,
                'result': result,
                'execution_time': step_end - step_start,
                'success': True
            })
        except Exception as e:
            step_end = time.time()
            
            execution_log.append({
                'step': i,
                'tool': tool.name,
                'error': str(e),
                'execution_time': step_end - step_start,
                'success': False
            })
    
    total_time = time.time() - start_time
    
    return {
        'result': execution_log[-1]['result'] if execution_log else None,
        'execution_log': execution_log,
        'total_time': total_time
    }
```

## 🔧 常见问题

### Q1: LangChain 工具和原生工具有什么区别？
**A**: LangChain 工具是原生工具的 LangChain 兼容版本，提供标准的 LangChain 工具接口。

### Q2: 如何选择工具链类型？
**A**: 
- 简单工具链：线性流程
- 条件工具链：需要分支逻辑
- 循环工具链：批量处理
- 复杂工具链：多种逻辑组合

### Q3: 工具链性能如何优化？
**A**: 
- 缓存工具对象
- 批量处理
- 并行执行
- 结果缓存

### Q4: 如何处理工具链错误？
**A**: 
```python
try:
    result = tool.func(input_data)
except Exception as e:
    # 错误处理
    print(f"工具调用失败: {e}")
    # 决定是否继续
```

### Q5: 如何监控工具链性能？
**A**: 
- 记录执行时间
- 监控工具调用
- 记录错误信息
- 生成性能报告

## 🔗 相关文档

- [LangChain 集成文档](../../../mcpstore_docs/docs/integrations/overview.md)
- [LangChain 工具列表文档](../../../mcpstore_docs/docs/tools/langchain/langchain-list-tools.md)
- [LangChain 使用示例文档](../../../mcpstore_docs/docs/tools/langchain/examples.md)
- [工具链构建文档](../../../mcpstore_docs/docs/advanced/chaining.md)

