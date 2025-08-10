# 🚀 McpStore：最好的mcp管理


## 快速使用

### 安装
```bash
pip install mcpstore
```


## 在线体验

本项目有一个示例的Vue的前端，你可以通过SDK或者Api的方式直观的管理你的MCP服务

![image-20250721212359929](http://www.text2mcp.com/img/image-20250721212359929.png)

通过一段简单的代码快速启动后端：

```python
from mcpstore import MCPStore
prod_store = MCPStore.setup_store()
prod_store.start_api_server(
    host='0.0.0.0',
    port=18200
)
```

通过 https://mcpstore.wiki/web_demo/dashboard 体验在线示例


通过 https://doc.mcpstore.wiki/ 可以查看详细的使用文档

## MCP 的工具即拿即用 ⚡

无需关注 `mcp` 层级的协议和配置，简单的使用直观的类和函数。

```python
store = MCPStore.setup_store()

store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})

tools = store.for_store().list_tools()

# store.for_store().use_tool(tools[0].name,{"query":'hi!'})
```



## 一个完整的可运行示例，直接使你的 langchain 使用 mcp 服务 🔥

下面是一个完整的、可直接运行的示例，展示了如何将 `McpStore` 获取的工具无缝集成到标准的 `langChain Agent` 中。

```python
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
from mcpstore import MCPStore
# ===
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
tools = store.for_store().for_langchain().list_tools()
# ===
llm = ChatOpenAI(
    temperature=0, model="deepseek-chat",
    openai_api_key="****",
    openai_api_base="https://api.deepseek.com"
)
prompt = ChatPromptTemplate.from_messages([
    ("system", "你是一个助手，回答的时候带上表情"),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}"),
])
agent = create_tool_calling_agent(llm, tools, prompt)
agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
# ===
query = "北京的天气怎么样？"
print(f"\n   🤔: {query}")
response = agent_executor.invoke({"input": query})
print(f"   🤖 : {response['output']}")
```


![image-20250721212658085](http://www.text2mcp.com/img/image-20250721212658085.png)



## 链式调用 ⛓️

本人讨厌复杂和超长的函数名，为了直观的展示代码，`McpStore` 采用的是 `链式`。

具体来说，`store` 是一个基石，在这个基础上，如果你有不同的 `agent`，你希望你的不同的 `agent` 是不同领域的专家（使用隔离的不同的 `MCP` 们），那么你可以试一下 `for_agent`.


每个 `agent` 之间是隔离的，你可以通过自定义一个 `agentid` 来确定你的 `agent` 的身份，并保证他只在他的范围内做的更好。


计划支持A2A协议，更好的集成A2ACard。


* `store.for_store()`：整个store空间。
* `store.for_agent("agent_id")`：为指定 ID 的 Agent 创建一个隔离的空间，是store的子集。
## 多 Agent 隔离

如何利用 `上下文隔离`，为不同职能的 Agent 分配 `专属的工具集`。
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


## API 🌐

MCPStore 提供`完备RESTful API`

`一行命令` 即可启动完整的 Web 服务：
```bash
pip install mcpstore
mcpstore run api
```
启动后立即获得API 接口 🚀

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

更多请见开发文档
通过 https://doc.mcpstore.wiki/ 可以查看详细的使用文档

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

**MCPStore是一个还在频繁的更新的项目，恳求大家给小星并来指点**

![image-20250810191737450](http://www.text2mcp.com/img/image-20250810191737450.png)
