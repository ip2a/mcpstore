# 🚀 McpStore 三行代码为你的Agent添加MCP能力

`McpStore` 是一个专为解决 Agent 想要使用 `MCP（Model Context Protocol）` 的能力，但是疲于管理 MCP 的工具管理库。

MCP快速发展,我们都想为现有的Agent添加MCP的能力，但是为Agent引入新工具通常需要编写大量重复的“胶水代码”，流程繁琐



## 三行代码实现将 MCP 的工具即拿即用 ⚡

无需关注 `mcp` 层级的协议和配置，只需要简单的使用直观的类和函数，提供 `极致简洁` 的用户体验。

```python
# 引入MCPStore库
from mcpstore import MCPStore
# 步骤1: 初始化一个Store，这是管理所有MCP服务的核心入口
store = MCPStore.setup_store()
# 步骤2: 注册一个外部MCP服务，MCPStore会自动处理连接和工具加载
store.for_store().add_service({"name":"mcpstore-wiki","url":"http://mcpstore.wiki/mcp"})
# 步骤3: 获取与LangChain完全兼容的工具列表，可直接用于Agent
tools = store.for_store().for_langchain().list_tools()
# 此刻，您的LangChain Agent已成功集成了mcpstore-wiki提供的所有工具
```



## 一个完整的可运行示例，直接使你的 langchain 使用 mcp 服务 🔥

下面是一个完整的、可直接运行的示例，展示了如何将 `McpStore` 获取的工具无缝集成到标准的 `langChain Agent` 中。

```python
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
from mcpstore import MCPStore
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"http://mcpstore.wiki/mcp"})
tools = store.for_store().to_langchain_tools()
llm = ChatOpenAI(
    temperature=0, model="deepseek-chat",
    openai_api_key="sk-****",
    openai_api_base="https://api.deepseek.com"
)
prompt = ChatPromptTemplate.from_messages([
    ("system", "你是一个助手，回答的时候带上表情"),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}"),
])
agent = create_tool_calling_agent(llm, tools, prompt)
agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
query = "北京的天气怎么样？"
print(f"\n   🤔: {query}")
response = agent_executor.invoke({"input": query})
print(f"   🤖 : {response['output']}")
```


![image-20250711002833332](./assets/image-20250711002833332.png)


或者你不想使用 `langchain`，你打算 `自己设计工具的调用` 🛠️

```
from mcpstore import MCPStore
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"http://mcpstore.wiki/mcp"})
tools = store.for_store().list_tools()
print(store.for_store().use_tool(tools[0].name,{"query":'北京'}))
```



## 快速上手

### 安装
```bash
pip install mcpstore
```


## 链式调用 ⛓️

本人很讨厌复杂和超长的函数名，为了直观的展示代码，`McpStore` 采用的是 `链式`。具体来说，`store` 是一个基石，在这个基础上，如果你有不同的 `agent`，你希望你的不同的 `agent` 是不同领域的专家（使用隔离的不同的 `MCP` 们），那么你可以试一下 `for_agent`，每个 `agent` 之间是隔离的，你可以通过自定义一个 `agentid` 来确定你的 `agent` 的身份，并保证他只在他的范围内做的更好。

* `store.for_store()`：进入 `全局上下文`，在此处管理的服务和工具对所有 Agent 可见。
* `store.for_agent("agent_id")`：为指定 ID 的 Agent 创建一个 `隔离的私有上下文`。每个


## 多 Agent 隔离的 🏠

以下代码演示了如何利用 `上下文隔离`，为不同职能的 Agent 分配 `专属的工具集`。
```python
# 初始化Store
store = MCPStore.setup_store()

# 为“知识管理Agent”分配专用的Wiki工具
# 该操作在"knowledge" agent的私有上下文中进行
agent_id1 = "my-knowledge-agent"
knowledge_agent_context = store.for_agent(agent_id1).add_service(
    {"name": "mcpstore-wiki", "url": "http://mcpstore.wiki/mcp"}
)

# 为“开发支持Agent”分配专用的开发工具
# 该操作在"development" agent的私有上下文中进行
agent_id2 = "my-development-agent"
dev_agent_context = store.for_agent(agent_id2).add_service(
    {"name": "mcpstore-demo", "url": "http://mcpstore.wiki/mcp"}
)

# 各Agent的工具集完全隔离，互不影响
knowledge_tools = store.for_agent(agent_id1).list_tools()
dev_tools = store.for_agent(agent_id2).list_tools()
```
很直观的，你可以通过 `store.for_store()` 和 `store.for_agent("agent_id")` 使用几乎所有的函数 ✨


## McpStore 的 setup_store() 🔧


### 📋 概述

`MCPStore.setup_store()` 是 MCPStore 的 `核心初始化方法`，用于创建和配置 MCPStore 实例。该方法支持 `自定义配置文件路径` 和 `调试模式`，为不同环境和使用场景提供 `灵活的配置选项`。

### 🔧 方法签名

```python
@staticmethod
def setup_store(mcp_config_file: str = None, debug: bool = False) -> MCPStore
```

**参数说明**:
- `mcp_config_file`: 自定义 mcp.json 配置文件路径（可选）
- `debug`: 是否启用调试日志模式（可选，默认 False）
- **返回值**: 完全初始化的 MCPStore 实例

### 📋 参数详解

#### 1. `mcp_config_file` 参数

- **未指定时**: 使用默认路径 `src/mcpstore/data/mcp.json`
- **指定时**: 使用指定的 `mcp.json` 配置文件来实例化你的 store，支持 `主流 client 的文件格式`，`拿来即用` 🎯

#### 2. `debug` 参数

##### 基本说明
- **类型**: `bool`
- **默认值**: `False`
- **作用**: 控制日志输出级别和详细程度

##### 日志配置对比

| 模式 | debug=False (默认) | debug=True |
|------|-------------------|------------|
| **日志级别** | ERROR | DEBUG |
| **日志格式** | `%(levelname)s - %(message)s` | `%(asctime)s - %(name)s - %(levelname)s - %(message)s` |
| **显示内容** | 只显示错误信息 | 显示所有调试信息 |


### 📁 支持的 JSON 配置格式

#### 标准 MCP 配置格式

MCPStore 使用 `标准的 MCP 配置格式`，支持 `URL 方式` 和 `命令方式` 的服务配置：

```json
{
  "mcpServers": {
    "mcpstore-wiki": {
      "url": "http://mcpstore.wiki/mcp"
    },
    "howtocook": {
      "command": "npx",
      "args": [
        "-y",
        "howtocook-mcp"
      ]
    }
  }
}
```


#### 场景：多租户配置 🏢

```python
# 租户 A 的配置
tenant_a_store = MCPStore.setup_store(
    mcp_config_file="tenant_a_mcp.json",
    debug=False
)

# 租户 B 的配置
tenant_b_store = MCPStore.setup_store(
    mcp_config_file="tenant_b_mcp.json",
    debug=False
)

# 为不同租户提供隔离的服务
tenant_a_tools = tenant_a_store.for_store().list_tools()
tenant_b_tools = tenant_b_store.for_store().list_tools()
```


## 强大的服务注册 `add_service` 💪

`mcpstore` 的核心是 `store`。只需通过 `setup_store()` 初始化一个的 `store`，就可以在这个 `store` 上注册 `任意数量`、支持所有 `MCP 协议` 的服务，不必担心各个 mcp 服务的 `生命周期和维护`，不必担心针对 mcp 服务的 `增删改查`，`store` 会 `全权负责` 这些服务的生命周期维护。

当需要将这些服务集成到 langchain Agent 中时，调用 `store.for_store().to_langchain_tools()` 即可 `一键转换` 为完全兼容 langchain `Tool` 结构的工具集，方便您直接使用或与现有工具 `无缝结合`。

或者可以直接使用 `store.for_store().use_tool()` 方法，`自定义你想要的工具调用` 🎯。

### 服务注册方式

所有通过 `add_service` 添加的服务，其配置都会被 `统一管理`，并可选择持久化到 setup_store 注册时的 `mcp.json` 文件中，`去重和更新` 会由 mcpstore `自动进行` ⚙️。
    
    
### 基本语法
```python
store = MCPStore.setup_store()
store.for_store().add_service(config)
```

### 支持的注册方式

#### 1. 🔄 全量注册（无参数）
注册 `mcp.json` 配置文件中的所有服务。

```python
store.for_store().add_service()
```
不传递任何参数，`add_service` 会 `自动查找并加载` 项目根目录下的 `mcp.json` 文件，该文件 `兼容主流格式`。

**使用场景**:
- 项目初始化时 `一次性注册` 所有预配置的服务
- `重新加载` 所有服务配置

---

#### 2. 🌐 URL 方式注册
通过 URL 添加远程 MCP 服务。

```python
store.for_store().add_service({
    "name": "mcpstore-wiki",
    "url": "http://mcpstore.wiki/mcp",
    "transport": "streamable-http"
})
```

**字段**:
- `name`: 服务名称
- `url`: 服务 URL
- `transport`: 可选字段，可以 `自动推断` 传输协议 (`streamable-http`, `sse`)

---

#### 3. 💻 本地命令方式注册
启动本地 MCP 服务进程。

```python
# Python 服务
store.for_store().add_service({
    "name": "local_assistant",
    "command": "python",
    "args": ["./assistant_server.py"],
    "env": {"DEBUG": "true", "API_KEY": "your_key"},
    "working_dir": "/path/to/service"
})

# Node.js 服务
store.for_store().add_service({
    "name": "node_service",
    "command": "node",
    "args": ["server.js", "--port", "8080"],
    "env": {"NODE_ENV": "production"}
})

# 可执行文件
store.for_store().add_service({
    "name": "binary_service",
    "command": "./mcp_server",
    "args": ["--config", "config.json"]
})
```

**必需字段**:
- `name`: 服务名称
- `command`: 执行命令

**可选字段**:
- `args`: 命令参数列表
- `env`: 环境变量字典
- `working_dir`: 工作目录

---

#### 4. 📄 MCPConfig 字典方式注册
使用标准 MCP 配置格式。

```python
store.for_store().add_service({
  "mcpServers": {
    "mcpstore-wiki": {
      "url": "http://mcpstore.wiki/mcp"
    },
    "howtocook": {
      "command": "npx",
      "args": [
        "-y",
        "howtocook-mcp"
      ]
    }
  }
})
```

---

#### 5. 📝 服务名称列表方式注册
从现有配置中选择特定服务注册。

```python
# 注册指定的服务
store.for_store().add_service(['mcpstore-wiki', 'howtocook'])

# 注册单个服务
store.for_store().add_service(['howtocook'])
```

**前提条件**: 服务必须已在 `mcp.json` 配置文件中定义 📋。

---

#### 6. 📁 JSON 文件方式注册
从外部 JSON 文件读取配置。

```python
# 从文件读取配置
store.for_store().add_service(json_file="./demo_config.json")

# 同时指定 config 和 json_file（优先使用 json_file）
store.for_store().add_service(
    config={"name": "backup"},
    json_file="./demo_config.json"  # 这个会被使用 ⚡
)
```

**JSON 文件格式示例**:
```json
{
  "mcpServers": {
    "mcpstore-wiki": {
      "url": "http://mcpstore.wiki/mcp"
    },
    "howtocook": {
      "command": "npx",
      "args": [
        "-y",
        "howtocook-mcp"
      ]
    }
  }
}
```
以及 `add_service` 支持的其他格式 📝

``` json
{
    "name": "mcpstore-wiki",
    "url": "http://mcpstore.wiki/mcp"
}
```

---


## RESTful API 🌐

除了作为 `Python 库` 使用，MCPStore 还提供了一套 `完备的 RESTful API`，让您可以将 `MCP 工具管理能力` 无缝集成到任何后端服务或管理平台中。

`一行命令` 即可启动完整的 Web 服务：
```bash
pip install mcpstore
mcpstore run api
```
启动后立即获得 `38个` API 接口 🚀

### 📡 完整的 API 生态

#### Store 级别 API 🏪

```bash
# 服务管理
POST /for_store/add_service          # 添加服务
GET  /for_store/list_services        # 获取服务列表
POST /for_store/delete_service       # 删除服务
POST /for_store/update_service       # 更新服务
POST /for_store/restart_service      # 重启服务

# 工具操作
GET  /for_store/list_tools           # 获取工具列表
POST /for_store/use_tool             # 执行工具

# 批量操作
POST /for_store/batch_add_services   # 批量添加
POST /for_store/batch_update_services # 批量更新

# 监控统计
GET  /for_store/get_stats            # 系统统计
GET  /for_store/health               # 健康检查
```

#### Agent 级别 API 🤖

```bash
# 完全对应Store级别，支持多租户隔离
POST /for_agent/{agent_id}/add_service
GET  /for_agent/{agent_id}/list_services
# ... 所有Store级别功能都支持
```

#### 监控系统 API（3个接口）📊

```bash
GET  /monitoring/status              # 获取监控状态
POST /monitoring/config              # 更新监控配置
POST /monitoring/restart             # 重启监控任务
```

#### 通用 API 🔧

```bash
GET  /services/{name}                # 跨上下文服务查询
```






## 开发者文档与资源 📚

### 详细的 API 接口文档
我们提供 `详尽的 RESTful API 文档`，旨在帮助开发者 `快速集成与调试`。文档为每个 API 端点提供了 `全面的信息`，包括：
* **功能描述**：接口的用途和业务逻辑。
* **URL与HTTP方法**：标准的请求路径和方法。
* **请求参数**：详细的输入参数说明、类型及校验规则。
* **响应示例**：清晰的成功与失败响应结构示例。
* **Curl调用示例**：可直接复制运行的命令行调用示例。
* **源码追溯**：关联到实现该接口的后端源码文件、类及关键函数，实现从 `API 到代码的透明化`，极大地方便了 `深度调试和问题定位` 🔍。

### 源码级开发文档 (LLM友好型) 🤖
为了支持 `深度定制和二次开发`，我们还提供了一份 `独特的源码级参考文档`。这份文档不仅 `系统性地梳理` 了项目中所有核心的类、属性及方法，更重要的是，我们额外提供了一份为 `大语言模型（LLM）优化` 的 `llm.txt` 版本。
开发者可以直接将这份 `纯文本格式` 的文档提供给 AI 模型，让 AI 辅助进行 `代码理解`、`功能扩展` 或 `重构`，从而实现真正的 `AI 驱动开发（AI-Driven Development）` ✨。

## 参与贡献 🤝

MCPStore 是一个 `开源项目`，我们欢迎社区的 `任何形式的贡献`：

* ⭐ 如果项目对您有帮助，请在 `GitHub` 上给我们一个 Star。
* 🐛 通过 `Issues` 提交错误报告或功能建议。
* 🔧 通过 `Pull Requests` 贡献您的代码。
* 💬 加入社区，分享您的 `使用经验` 和 `最佳实践`。

---

**MCPStore：让 MCP 工具管理变得 `简单而强大` 💪。**
