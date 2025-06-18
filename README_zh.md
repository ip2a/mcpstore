# MCPStore

MCPStore 是一个强大轻量级的 MCP（Model Context Protocol）工具管理库。
该包的开发初衷是解决对于许多的 agent 或者 chain 来说，我们想要使用 MCP 的
tool，但是对于每个agent都配置MCP 和管理有些复杂。针对这个情况，我开发了 MCPStore。对于智能体来说，我们相当于创建了一个 store，agent
可以挑选他需要的 MCP 服务。我的目的是让现有的 agent 开发项目可以无感添加 tool，只需要几行代码的配置，就可以在原来的代码上添加这些工具。

## 特性

- 🚀 简单集成：仅需几行代码即可完成工具调用
- 🔄 链式操作：直观的 API 设计，支持流畅的链式调用
- 🎯 精确控制：支持全局 Store 模式和独立 Agent 模式
- 🔒 隔离管理：不同 Agent 之间的服务和工具完全隔离
- 📦 配置集中：统一的配置管理，支持动态服务注册

## 快速开始

### 安装

```bash
pip install mcpstore
```

### 快速使用

只需三行代码即可实现工具调用。支持多种方式：



```python
# 1. 创建 Store 实例
from mcpstore import MCPStore

store = MCPStore.setup_store()

# 2. 注册配置文件中的服务
reg_result = await store.for_store().add_service({"map": {    "url": "https://mcp.amap.com/sse?key=YourKey"}})

# 3. 使用工具
result = await store.for_store().use_tool(    "map_maps_direction_driving",    {        "origin": "116.481028,39.989643",        "destination": "116.434446,39.90816"    })
```



### 服务注册方式

MCPStore 有强大的 `add_service` 来添加服务：
在MCPStore中，有Store和Agent的概念，store即帮你注册和维护你的mcp服务器的单位，你可以使用
store.for_store().add_service()
不传参数直接为store注册你的mcp.json文件，该文件支持cursor等主流的文件格式

也可以使用
   await store.for_store().add_service({
       "name": "weather",
       "url": "https://weather-api.example.com/mcp",
       "transport": "streamable-http"  # 或 "sse"
   })

   # 本地命令方式
   await store.for_store().add_service({
       "name": "assistant",
       "command": "python",
       "args": ["./assistant_server.py"],
       "env": {"DEBUG": "true"}
   })

   # MCPConfig字典方式
   await store.for_store().add_service({
       "mcpServers": {
           "weather": {
               "url": "https://weather-api.example.com/mcp"
           }
       }
   })
来帮你添加指定的服务，并且内容会同步到mcp.json中


对于agent是类似的 你只需通过for_agent和for_store就可以灵活的切换作用域
agent除了支持上述的几种方法添加mcp服务外，还支持通过已有的mcpname添加服务
   # 服务名称列表方式
   await store.for_agent("agent123").add_service(['weather', 'assistant'])
for_agent模式下 相当于是从store中挑选mcp服务，初衷是为了不同的智能体不需要那么多的工具混淆智能体的特长，
你可以自定义agent的id，store会记住你需要哪些mcp服务，如果你使用
for_agent(agent_id).list_tools()或者for_agent(agent_id).list_services()
你都能轻松的找到他们




#### 配置同步机制

- 所有通过 `add_service` 添加的服务配置都会自动同步到 mcp.json 文件
- Store 模式下添加的服务对所有 Agent 可见
- Agent 模式下添加的服务会：
    - 更新到 mcp.json（如果是新服务）
    - 在 agent_clients.json 中创建 Agent-Client 映射
    - 在 client_services.json 中添加客户端配置

#### 最佳实践

1. Store 模式使用建议：
    - 全局服务优先使用配置文件注册
    - 动态服务使用直接配置方式添加

2. Agent 模式使用建议：
    - 已有服务使用服务名称列表注册
    - 特定服务使用直接配置方式添加
    - 注意服务隔离，避免相互影响

3. 配置管理：
    - 定期检查配置文件同步状态
    - 重要配置变更前备份配置文件
    - 使用健康检查确保服务可用

## 使用场景

我采用直观的方法来设计 store，当你执行 `store = MCPStore.setup_store()` 之后你就拥有了一个 store，此时你可以围绕 store
进行各种操作。

### Store 模式（全局工具管理）

Store 模式下，你可以进行链式操作，代码示例：

```python
# 初始化 store
store = MCPStore.setup_store()

print('=== 1. 链式store操作 ===')
# 注册（全量）
reg_result = await store.for_store().add_service()
print('[链式store] 注册结果:', reg_result)

# 列出服务
services = await store.for_store().list_services()
print('[链式store] 服务列表:', services)

# 列出工具
tools = await store.for_store().list_tools()
print('[链式store] 工具列表:', tools)

# 健康检查
health = await store.for_store().check_services()
print('[链式store] 健康检查:', health)


 detail = await store.get_service_info(your_services_name)
 print(f'[链式store] 服务详情:', detail)

# 使用工具示例
result = await store.for_store().use_tool(
    "map_maps_direction_driving",
    {
        "origin": "116.481028,39.989643",
        "destination": "116.434446,39.90816"
    }
)
print('[链式store] 驾车导航结果:', result)
```

### Agent 模式（独立工具管理）

对于 agent 来说，如果你不希望 agent 添加所有的 MCP 工具，你希望你的 agent 可以是某一个行业的专家，你只需要指定一个
id，或者自动创建一个 id，然后你就可以对这个 agent 进行隔离的服务调用和执行。示例：

```python
print('\n=== 2. 链式agent操作 ===')
agent_id = 'agent123'

# 注册指定服务
reg_result = await store.for_agent(agent_id).add_service(['高德'])
print('[链式agent] 注册结果:', reg_result)

# 列出服务
agent_services = await store.for_agent(agent_id).list_services()
print('[链式agent] 服务列表:', agent_services)

# 列出工具
agent_tools = await store.for_agent(agent_id).list_tools()
print('[链式agent] 工具列表:', agent_tools)

# 健康检查
agent_health = await store.for_agent(agent_id).check_services()
print('[链式agent] 健康检查:', agent_health)

# 展示单个服务详情
if agent_services:
    detail = await store.get_service_info(agent_services[0].name)
    print(f'[链式agent] 服务详情:', detail)

# Agent工具调用示例
agent_result = await store.for_agent(agent_id).use_tool(
    "高德_maps_direction_walking",
    {
        "origin": "116.481028,39.989643",
        "destination": "116.434446,39.90816"
    }
)
print('[链式agent] 步行导航结果:', agent_result)
```

### 配置文件

所有配置文件统一存放在 `data/defaults` 目录下：

- `mcp.json`: MCP 服务配置
- `client_services.json`: 客户端服务配置
- `agent_clients.json`: Agent-Client 映射配置

## API 参考

### Store API

- `for_store()`: 进入 Store 上下文
- `add_service()`: 注册服务
- `list_services()`: 列出服务
- `list_tools()`: 列出工具
- `check_services()`: 健康检查
- `use_tool()`: 调用工具

### Agent API

- `for_agent(agent_id)`: 进入 Agent 上下文
- `add_service(service_list)`: 注册指定服务
- `list_services()`: 列出 Agent 可用服务
- `list_tools()`: 列出 Agent 可用工具
- `check_services()`: Agent 服务健康检查
- `use_tool()`: 调用 Agent 可用工具

## 贡献指南

欢迎提交 Issue 和 Pull Request 来帮助改进 MCPStore。

## 近期计划更新 🚀

### API 增强

- [ ] 完善现有 API 的参数验证和错误处理
- [ ] 添加更多实用的工具方法
- [ ] 提供更灵活的配置选项
- [ ] 支持异步批量操作

### 服务注册增强

- [ ] 增强 `add_service` 的容错能力
- [ ] 支持多种服务注册模式（单个、批量、条件注册）
- [ ] 添加服务注册状态监控
- [ ] 支持服务热更新
- [ ] 支持自定义重试策略

### LangChain 集成

- [ ] 提供与 LangChain 的无缝集成接口
- [ ] 支持 LangChain Agent 工具链
- [ ] 实现 LangChain 工具的自动转换
- [ ] 提供标准的 LangChain 工具模板

### 配置文件管理

- [ ] 增强 JSON 配置文件的处理能力
- [ ] 支持配置文件的导入导出
- [ ] 添加配置文件的版本控制
- [ ] 提供配置文件的验证工具
- [ ] 支持配置文件的动态更新
- [ ] 添加配置文件的备份和恢复功能

### 开发者工具

- [ ] 提供更详细的调试信息
- [ ] 添加性能分析工具
- [ ] 提供服务测试工具集
- [ ] 完善开发文档


