# 工具详情测试模块

本模块包含工具详细信息查询相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_tool_detail_info.py` | Store 获取工具详细信息 | Store 级别 |
| `test_store_tool_detail_tags.py` | Store 获取工具标签 | Store 级别 |
| `test_store_tool_detail_schema.py` | Store 获取工具输入模式 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 获取工具详细信息
python example/tool/detail/test_store_tool_detail_info.py

# 获取工具标签
python example/tool/detail/test_store_tool_detail_tags.py

# 获取工具输入模式
python example/tool/detail/test_store_tool_detail_schema.py
```

### 运行所有工具详情测试

```bash
# Windows
for %f in (example\tool\detail\test_*.py) do python %f

# Linux/Mac
for f in example/tool/detail/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 获取工具详细信息
测试 `tool_info()` 方法：
- 获取工具的完整信息
- 展示名称、描述、输入模式
- 查看所属服务
- 对比多个工具的信息

### 2. Store 获取工具标签
测试 `tool_tags()` 方法：
- 获取工具标签
- 标签格式（列表/字符串/字典）
- 使用标签进行工具分类
- 标签的实际应用

### 3. Store 获取工具输入模式
测试 `tool_schema()` 方法：
- 获取工具输入参数模式
- 解析 JSON Schema
- 生成调用示例
- 参数验证和文档生成

## 💡 核心概念

### 三种详情方法

| 方法 | 返回内容 | 用途 | 示例 |
|------|----------|------|------|
| `tool_info()` | 完整工具信息 | 查看工具详情 | 名称、描述、模式 |
| `tool_tags()` | 工具标签 | 分类和过滤 | 标签列表 |
| `tool_schema()` | 输入参数模式 | 参数验证 | JSON Schema |

### tool_info() 返回结构

```python
info = tool_proxy.tool_info()

# 典型结构
{
    "name": "get_current_weather",
    "description": "获取指定城市的当前天气",
    "inputSchema": {
        "type": "object",
        "properties": {...}
    },
    "service": "weather",
    "tags": ["weather", "api"]
}
```

### tool_schema() 返回结构

```python
schema = tool_proxy.tool_schema()

# JSON Schema 格式
{
    "type": "object",
    "properties": {
        "query": {
            "type": "string",
            "description": "城市名称"
        }
    },
    "required": ["query"]
}
```

## 🎯 使用场景

### 场景 1：查看工具详情
```python
tool = store.for_store().find_tool("get_weather")

# 获取完整信息
info = tool.tool_info()
print(f"工具名称: {info['name']}")
print(f"工具描述: {info['description']}")
```

### 场景 2：标签过滤
```python
# 获取所有工具
tools = store.for_store().list_tools()

# 按标签过滤
weather_tools = []
for tool in tools:
    proxy = store.for_store().find_tool(tool.name)
    tags = proxy.tool_tags()
    if tags and 'weather' in tags:
        weather_tools.append(tool.name)

print(f"天气相关工具: {weather_tools}")
```

### 场景 3：参数验证
```python
tool = store.for_store().find_tool("get_weather")

# 获取输入模式
schema = tool.tool_schema()

# 验证参数
def validate_params(params, schema):
    required = schema.get('required', [])
    for field in required:
        if field not in params:
            raise ValueError(f"缺少必填参数: {field}")
    return True

# 调用前验证
params = {"query": "北京"}
if validate_params(params, schema):
    result = tool.call_tool(params)
```

### 场景 4：动态UI生成
```python
# 根据 schema 生成表单
schema = tool.tool_schema()
properties = schema.get('properties', {})

for field_name, field_schema in properties.items():
    field_type = field_schema.get('type')
    description = field_schema.get('description')
    required = field_name in schema.get('required', [])
    
    # 生成对应的表单组件
    print(f"字段: {field_name}")
    print(f"类型: {field_type}")
    print(f"说明: {description}")
    print(f"必填: {'是' if required else '否'}")
```

## 📊 信息对比

### tool_info() vs tool_schema()

| 方面 | tool_info() | tool_schema() |
|------|-------------|---------------|
| **内容** | 完整工具信息 | 输入参数模式 |
| **格式** | 自定义字典 | JSON Schema |
| **用途** | 展示和文档 | 参数验证 |
| **包含** | name, description, schema | properties, required, type |

## 💡 最佳实践

### 1. 信息缓存
```python
# 缓存工具信息
tool_info_cache = {}

def get_tool_info_cached(tool_name):
    if tool_name not in tool_info_cache:
        tool = store.for_store().find_tool(tool_name)
        tool_info_cache[tool_name] = tool.tool_info()
    return tool_info_cache[tool_name]
```

### 2. 生成工具文档
```python
def generate_tool_doc(tool_name):
    """生成工具文档"""
    tool = store.for_store().find_tool(tool_name)
    
    # 获取信息
    info = tool.tool_info()
    schema = tool.tool_schema()
    
    # 生成文档
    doc = f"# {info['name']}\n\n"
    doc += f"{info['description']}\n\n"
    doc += "## 参数\n\n"
    
    if 'properties' in schema:
        for prop_name, prop_schema in schema['properties'].items():
            doc += f"- **{prop_name}** ({prop_schema.get('type')}): "
            doc += f"{prop_schema.get('description', 'N/A')}\n"
    
    return doc
```

### 3. 参数自动补全
```python
def get_param_suggestions(tool_name):
    """获取参数建议"""
    tool = store.for_store().find_tool(tool_name)
    schema = tool.tool_schema()
    
    suggestions = {}
    if 'properties' in schema:
        for prop_name, prop_schema in schema['properties'].items():
            suggestions[prop_name] = {
                'type': prop_schema.get('type'),
                'description': prop_schema.get('description'),
                'required': prop_name in schema.get('required', [])
            }
    
    return suggestions
```

### 4. 标签管理
```python
def group_tools_by_tag():
    """按标签分组工具"""
    tools = store.for_store().list_tools()
    tag_groups = {}
    
    for tool in tools:
        proxy = store.for_store().find_tool(tool.name)
        tags = proxy.tool_tags()
        
        if not tags:
            tags = ['untagged']
        elif isinstance(tags, str):
            tags = [tags]
        
        for tag in tags:
            if tag not in tag_groups:
                tag_groups[tag] = []
            tag_groups[tag].append(tool.name)
    
    return tag_groups
```

## 🔧 常见问题

### Q1: tool_info() 和 tool_schema() 的区别？
**A**: 
- `tool_info()`: 返回完整信息（包括 schema）
- `tool_schema()`: 只返回输入参数模式
- 如果只需要参数信息，用 `tool_schema()` 更轻量

### Q2: 标签是必须的吗？
**A**: 不是。标签是可选的元数据，用于工具分类和组织。

### Q3: schema 的格式是什么？
**A**: 通常是 JSON Schema 格式，包含：
- `type`: 数据类型
- `properties`: 属性定义
- `required`: 必填字段列表

### Q4: 如何处理没有 schema 的工具？
**A**: 
```python
schema = tool.tool_schema()
if not schema or not schema.get('properties'):
    print("工具无输入参数")
else:
    # 处理参数
    pass
```

## 🔗 相关文档

- [tool_info() 文档](../../../mcpstore_docs/docs/tools/details/tool-info.md)
- [tool_tags() 文档](../../../mcpstore_docs/docs/tools/details/tool-tags.md)
- [tool_schema() 文档](../../../mcpstore_docs/docs/tools/details/tool-schema.md)
- [ToolProxy 文档](../../../mcpstore_docs/docs/tools/finding/tool-proxy.md)

