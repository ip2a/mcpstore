# 工具统计测试模块

本模块包含工具统计相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_tool_stats_usage.py` | Store 获取工具使用统计 | Store 级别 |
| `test_store_tool_stats_history.py` | Store 获取工具调用历史 | Store 级别 |
| `test_store_tool_stats_service.py` | Store 获取服务工具统计 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 获取工具使用统计
python example/tool/stats/test_store_tool_stats_usage.py

# Store 获取工具调用历史
python example/tool/stats/test_store_tool_stats_history.py

# Store 获取服务工具统计
python example/tool/stats/test_store_tool_stats_service.py
```

### 运行所有工具统计测试

```bash
# Windows
for %f in (example\tool\stats\test_*.py) do python %f

# Linux/Mac
for f in example/tool/stats/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 获取工具使用统计
测试 `usage_stats()` 方法：
- 获取工具使用统计信息
- 统计信息更新测试
- 多工具统计对比
- 统计信息分析

### 2. Store 获取工具调用历史
测试 `call_history()` 方法：
- 获取工具调用历史记录
- 历史记录更新测试
- 历史记录分析
- 多工具历史对比

### 3. Store 获取服务工具统计
测试 `tools_stats()` 方法：
- 获取服务中所有工具的统计
- 服务级统计信息
- 工具统计对比
- 统计信息结构分析

## 💡 核心概念

### 三种统计方法

| 方法 | 作用对象 | 返回内容 | 用途 | 示例 |
|------|----------|----------|------|------|
| `usage_stats()` | 单个工具 | 工具使用统计 | 工具监控 | 调用次数 |
| `call_history()` | 单个工具 | 调用历史记录 | 调试分析 | 调用详情 |
| `tools_stats()` | 服务 | 所有工具统计 | 服务监控 | 整体统计 |

### 统计信息类型

| 类型 | 内容 | 用途 | 更新频率 |
|------|------|------|----------|
| **使用统计** | 调用次数、性能指标 | 监控优化 | 实时 |
| **调用历史** | 参数、结果、时间戳 | 调试分析 | 实时 |
| **服务统计** | 整体工具使用情况 | 服务管理 | 实时 |

## 🎯 使用场景

### 场景 1：工具使用监控
```python
# 监控工具使用情况
def monitor_tool_usage():
    tools = store.for_store().list_tools()
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        stats = proxy.usage_stats()
        
        print(f"工具 {tool.name}:")
        print(f"  使用统计: {stats}")
        
        # 检查使用频率
        if isinstance(stats, dict) and 'call_count' in stats:
            if stats['call_count'] > 100:
                print(f"  ⚠️ 高频使用工具")
            elif stats['call_count'] == 0:
                print(f"  ⚠️ 未使用工具")
```

### 场景 2：性能分析
```python
# 分析工具性能
def analyze_tool_performance():
    tools = store.for_store().list_tools()
    performance_report = {}
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        stats = proxy.usage_stats()
        
        if isinstance(stats, dict):
            performance_report[tool.name] = {
                'call_count': stats.get('call_count', 0),
                'avg_response_time': stats.get('avg_response_time', 0),
                'success_rate': stats.get('success_rate', 0)
            }
    
    return performance_report
```

### 场景 3：调试工具调用
```python
# 调试工具调用问题
def debug_tool_calls(tool_name):
    tool = store.for_store().find_tool(tool_name)
    history = tool.call_history()
    
    print(f"工具 {tool_name} 调用历史:")
    for i, record in enumerate(history, 1):
        print(f"  调用 {i}:")
        print(f"    参数: {record.get('params', 'N/A')}")
        print(f"    结果: {record.get('result', 'N/A')}")
        print(f"    时间: {record.get('timestamp', 'N/A')}")
        print(f"    状态: {record.get('status', 'N/A')}")
```

### 场景 4：服务级监控
```python
# 服务级工具监控
def monitor_service_tools(service_name):
    service = store.for_store().find_service(service_name)
    stats = service.tools_stats()
    
    print(f"服务 {service_name} 工具统计:")
    print(f"  总工具数: {stats.get('total_tools', 0)}")
    print(f"  总调用数: {stats.get('total_calls', 0)}")
    
    if 'tools' in stats:
        print(f"  工具详情:")
        for tool_name, tool_stats in stats['tools'].items():
            print(f"    {tool_name}: {tool_stats}")
```

## 📊 统计信息对比

### 单个工具 vs 服务统计

| 方面 | 单个工具统计 | 服务统计 |
|------|-------------|----------|
| **范围** | 单个工具 | 所有工具 |
| **内容** | 详细统计 | 整体统计 |
| **用途** | 工具优化 | 服务管理 |
| **更新** | 实时 | 实时 |

### 使用统计 vs 调用历史

| 方面 | 使用统计 | 调用历史 |
|------|----------|----------|
| **内容** | 汇总数据 | 详细记录 |
| **用途** | 监控分析 | 调试分析 |
| **存储** | 统计信息 | 完整记录 |
| **性能** | 轻量 | 重量 |

## 💡 最佳实践

### 1. 统计信息缓存
```python
class ToolStatsCache:
    """工具统计缓存"""
    
    def __init__(self, store):
        self.store = store
        self.cache = {}
        self.cache_time = {}
        self.cache_ttl = 60  # 60秒缓存
    
    def get_tool_stats(self, tool_name):
        """获取工具统计（带缓存）"""
        import time
        
        current_time = time.time()
        if (tool_name in self.cache and 
            tool_name in self.cache_time and
            current_time - self.cache_time[tool_name] < self.cache_ttl):
            return self.cache[tool_name]
        
        # 更新缓存
        tool = self.store.for_store().find_tool(tool_name)
        stats = tool.usage_stats()
        
        self.cache[tool_name] = stats
        self.cache_time[tool_name] = current_time
        
        return stats
```

### 2. 统计信息聚合
```python
def aggregate_tool_stats():
    """聚合工具统计信息"""
    tools = store.for_store().list_tools()
    aggregated_stats = {
        'total_tools': len(tools),
        'total_calls': 0,
        'active_tools': 0,
        'tool_stats': {}
    }
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        stats = proxy.usage_stats()
        
        if isinstance(stats, dict):
            call_count = stats.get('call_count', 0)
            aggregated_stats['total_calls'] += call_count
            
            if call_count > 0:
                aggregated_stats['active_tools'] += 1
            
            aggregated_stats['tool_stats'][tool.name] = stats
    
    return aggregated_stats
```

### 3. 历史记录分析
```python
def analyze_call_history(tool_name):
    """分析工具调用历史"""
    tool = store.for_store().find_tool(tool_name)
    history = tool.call_history()
    
    if not history:
        return {"error": "无调用历史"}
    
    analysis = {
        'total_calls': len(history),
        'successful_calls': 0,
        'failed_calls': 0,
        'avg_response_time': 0,
        'common_params': {}
    }
    
    response_times = []
    
    for record in history:
        if isinstance(record, dict):
            # 统计成功/失败
            if record.get('status') == 'success':
                analysis['successful_calls'] += 1
            else:
                analysis['failed_calls'] += 1
            
            # 收集响应时间
            if 'response_time' in record:
                response_times.append(record['response_time'])
            
            # 统计常用参数
            params = record.get('params', {})
            for key, value in params.items():
                if key not in analysis['common_params']:
                    analysis['common_params'][key] = {}
                if value not in analysis['common_params'][key]:
                    analysis['common_params'][key][value] = 0
                analysis['common_params'][key][value] += 1
    
    # 计算平均响应时间
    if response_times:
        analysis['avg_response_time'] = sum(response_times) / len(response_times)
    
    return analysis
```

### 4. 统计信息报告
```python
def generate_stats_report():
    """生成统计信息报告"""
    tools = store.for_store().list_tools()
    report = {
        'timestamp': time.time(),
        'summary': {},
        'details': {}
    }
    
    # 生成摘要
    report['summary'] = {
        'total_tools': len(tools),
        'active_tools': 0,
        'total_calls': 0
    }
    
    # 生成详情
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        stats = proxy.usage_stats()
        
        if isinstance(stats, dict):
            call_count = stats.get('call_count', 0)
            report['summary']['total_calls'] += call_count
            
            if call_count > 0:
                report['summary']['active_tools'] += 1
            
            report['details'][tool.name] = stats
    
    return report
```

## 🔧 常见问题

### Q1: 统计信息是实时的吗？
**A**: 是的，统计信息会实时更新，每次工具调用后都会更新相关统计。

### Q2: 调用历史会保存多久？
**A**: 调用历史的保存时间取决于配置，通常会有一定的保留期限。

### Q3: 如何清理统计信息？
**A**: 统计信息通常会自动清理，也可以通过相关API手动清理。

### Q4: 统计信息影响性能吗？
**A**: 统计信息收集对性能影响很小，但调用历史可能占用较多存储空间。

### Q5: 如何导出统计信息？
**A**: 
```python
# 导出统计信息
def export_stats():
    tools = store.for_store().list_tools()
    stats_data = {}
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        stats_data[tool.name] = {
            'usage_stats': proxy.usage_stats(),
            'call_history': proxy.call_history()
        }
    
    return stats_data
```

## 🔗 相关文档

- [usage_stats() 文档](../../../mcpstore_docs/docs/tools/stats/usage-stats.md)
- [call_history() 文档](../../../mcpstore_docs/docs/tools/stats/call-history.md)
- [tools_stats() 文档](../../../mcpstore_docs/docs/tools/stats/tools-stats.md)
- [ToolProxy 文档](../../../mcpstore_docs/docs/tools/finding/tool-proxy.md)

