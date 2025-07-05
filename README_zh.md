# 🚀 McpStore 三行代码为你的Agent添加MCP能力

McpStore 是一个专为解决Agent想要使用MCP（Model Context Protocol）的能力，但是疲于管理MCP的工具管理库。

通常，随着MCP的快速发展,我们都想为现有的Agent添加这部分的能力，但是为Agent引入新工具通常需要编写大量重复的“胶水代码”，流程繁琐且效率低下。并且对多个MCP服务的生命周期（注册、发现、更新、注销）进行有效管理比较麻烦。

现在这些问题都将被优雅的解决

## 三行代码实现将MCP的工具拿出来使用

用户无需关注mcp层级的协议和配置，只需要简单的使用直观的类和函数，提供极致简洁的用户体验。

```python
# 引入MCPStore库
from mcpstore import MCPStore

# 步骤1: 初始化Store，这是管理所有MCP服务的核心入口
store = MCPStore.setup_store()

# 步骤2: 注册一个外部MCP服务，MCPStore会自动处理连接和工具加载
store.for_store().add_service({"name": "mcpstore-wiki", "url": "http://59.110.160.18:21923/mcp"})

# 步骤3: 获取与LangChain完全兼容的工具列表，可直接用于Agent
tools = store.for_store().for_langchain().list_tools()

# 此刻，您的LangChain Agent已成功集成了mcpstore-wiki提供的所有工具
```

##  一个完整的可运行示例，直接使你的langchain使用mcp服务

下面是一个完整的、可直接运行的示例，展示了如何将MCPStore获取的工具无缝集成到标准的LangChain Agent中。

```python
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
from mcpstore import MCPStore
store = MCPStore.setup_store()
store.for_store().add_service({"name": "mcpstore-wiki", "url": "http://59.110.160.18:21923/mcp"})
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

## 强大的服务注册 `add_service`

mcpstore的核心理念是，你可以通过setup_store()创建一个store，通过在这个store上注册mcp服务（支持所有的mcp协议），store会负责维护这些mcp服务，你只需要添加服务再添加服务，在给你的Agent使用之前，使用tools = store.for_store().to_langchain_tools()将tools传给langchain就可以，这个tools是完全兼容langchain的Tool结构的你可以直接使用，也可以和你的现有的langchain服务搭配使用

### 服务注册方式

`add_service` 支持多种参数格式，以适应不同的使用场景：

* **从配置文件加载**：
    不传递任何参数，`add_service` 会自动查找并加载项目根目录下的 `mcp.json` 文件，该文件兼容主流格式。
    ```python
    # 自动加载 mcp.json
    store.for_store().add_service()
    ```

* **通过URL注册**：
    最常见的方式，直接提供服务的名称和URL。MCPStore会自动推断传输协议。
    ```python
    # 通过网络地址添加服务
    store.for_store().add_service({
       "name": "weather",
       "url": "https://weather-api.example.com/mcp",
       "transport": "streamable-http"  # transport 可选，会自动推断
    })
    ```

* **通过本地命令启动**：
    对于本地脚本或可执行文件提供的服务，可以直接指定启动命令。
    ```python
    # 将本地Python脚本作为服务启动
    store.for_store().add_service({
       "name": "assistant",
       "command": "python",
       "args": ["./assistant_server.py"],
       "env": {"DEBUG": "true"}
    })
    ```

* **通过字典配置注册**：
    支持直接传入符合MCPConfig规范的字典结构。
    ```python
    # 以MCPConfig字典格式添加服务
    store.for_store().add_service({
       "mcpServers": {
           "weather": {
               "url": "https://weather-api.example.com/mcp"
           }
       }
    })
    ```
    所有通过 `add_service` 添加的服务，其配置都会被统一管理，并可选择持久化到 `mcp.json` 文件中。

##  RESTful API

除了作为Python库使用，MCPStore还提供了一套完备的RESTful API，让您可以将MCP工具管理能力无缝集成到任何后端服务或管理平台中。

一行命令即可启动完整的Web服务：
```bash
pip install mcpstore
mcpstore run api
```
启动后立即获得 **38个** API接口

### 📡 完整的API生态

#### Store级别API 

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

#### Agent级别API

```bash
# 完全对应Store级别，支持多租户隔离
POST /for_agent/{agent_id}/add_service
GET  /for_agent/{agent_id}/list_services
# ... 所有Store级别功能都支持
```

#### 监控系统API（3个接口）

```bash
GET  /monitoring/status              # 获取监控状态
POST /monitoring/config              # 更新监控配置
POST /monitoring/restart             # 重启监控任务
```

#### 通用API

```bash
GET  /services/{name}                # 跨上下文服务查询
```

##  链式调用与上下文管理

MCPStore采用富有表现力的链式API设计，使代码逻辑更加清晰、易读。同时，通过**上下文隔离（Context Isolation）**机制，为不同的Agent或全局Store提供独立且安全的服务管理空间。

* `store.for_store()`：进入全局上下文，在此处管理的服务和工具对所有Agent可见。
* `store.for_agent("agent_id")`：为指定ID的Agent创建一个隔离的私有上下文。每个Agent的工具集互不干扰，是实现多租户和复杂Agent系统的关键。

### 多Agent隔离的

以下代码演示了如何利用上下文隔离，为不同职能的Agent分配专属的工具集。
```python
# 初始化Store
store = MCPStore.setup_store()

# 为“知识管理Agent”分配专用的Wiki工具
# 该操作在"knowledge" agent的私有上下文中进行
agent_id1 = "my-knowledge-agent"
knowledge_agent_context = store.for_agent(agent_id1).add_service(
    {"name": "mcpstore-wiki", "url": "http://59.110.160.18:21923/mcp"}
)

# 为“开发支持Agent”分配专用的开发工具
# 该操作在"development" agent的私有上下文中进行
agent_id2 = "my-development-agent"
dev_agent_context = store.for_agent(agent_id2).add_service(
    {"name": "mcpstore-demo", "url": "http://59.110.160.18:21924/mcp"}
)

# 各Agent的工具集完全隔离，互不影响
knowledge_tools = store.for_agent(agent_id1).list_tools()
dev_tools = store.for_agent(agent_id2).list_tools()
```



## 安装与快速上手

### 安装
```bash
pip install mcpstore
```
### 快速启动
```bash
# 启动功能完备的API服务
mcpstore run api

# 在另一个终端，访问监控面板获取系统状态
curl http://localhost:18611/monitoring/status

# 测试添加一个MCP服务
curl -X POST http://localhost:18611/for_store/add_service \
  -H "Content-Type: application/json" \
  -d '{"name": "mcpstore-wiki", "url": "http://59.110.160.18:21923/mcp"}'
```



## 开发者文档与资源

### 详细的API接口文档
我们提供详尽的 RESTful API 文档，旨在帮助开发者快速集成与调试。文档为每个API端点提供了全面的信息，包括：
* **功能描述**：接口的用途和业务逻辑。
* **URL与HTTP方法**：标准的请求路径和方法。
* **请求参数**：详细的输入参数说明、类型及校验规则。
* **响应示例**：清晰的成功与失败响应结构示例。
* **Curl调用示例**：可直接复制运行的命令行调用示例。
* **源码追溯**：关联到实现该接口的后端源码文件、类及关键函数，实现从API到代码的透明化，极大地方便了深度调试和问题定位。

### 源码级开发文档 (LLM友好型)
为了支持深度定制和二次开发，我们还提供了一份独特的源码级参考文档。这份文档不仅系统性地梳理了项目中所有核心的类、属性及方法，更重要的是，我们额外提供了一份为大语言模型（LLM）优化的 `llm.txt` 版本。
开发者可以直接将这份纯文本格式的文档提供给AI模型，让AI辅助进行代码理解、功能扩展或重构，从而实现真正的AI驱动开发（AI-Driven Development）。

## 参与贡献

MCPStore是一个开源项目，我们欢迎社区的任何形式的贡献：

* ⭐ 如果项目对您有帮助，请在 **GitHub** 上给我们一个Star。
* 🐛 通过 **Issues** 提交错误报告或功能建议。
* 🔧 通过 **Pull Requests** 贡献您的代码。
* 💬 加入社区，分享您的使用经验和最佳实践。

---

**MCPStore：让MCP工具管理变得简单而强大。**
