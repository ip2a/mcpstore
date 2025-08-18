<div align="center">

# McpStore

一站式开源高质量MCP服务管理工具，让AI Agent轻松使用各种工具

![GitHub stars](https://img.shields.io/github/stars/whillhill/mcpstore) ![GitHub forks](https://img.shields.io/github/forks/whillhill/mcpstore) ![GitHub issues](https://img.shields.io/github/issues/whillhill/mcpstore) ![GitHub license](https://img.shields.io/github/license/whillhill/mcpstore) ![PyPI version](https://img.shields.io/pypi/v/mcpstore) ![Python versions](https://img.shields.io/pypi/pyversions/mcpstore) ![PyPI downloads](https://img.shields.io/pypi/dm/mcpstore?label=downloads)

[English](README.md) | 简体中文

🚀 [在线体验](https://mcpstore.wiki/web_demo/dashboard) | 📖 [详细文档](https://doc.mcpstore.wiki/) | 🎯 [快速开始](#快速使用)

</div>

## 快速开始

### 安装
```bash
pip install mcpstore
```

### 在线体验

开源的Vue前端界面，支持通过SDK或API方式直观管理MCP服务

![image-20250721212359929](http://www.text2mcp.com/img/image-20250721212359929.png)

快速启动后端服务：

```python
from mcpstore import MCPStore
prod_store = MCPStore.setup_store()
prod_store.start_api_server(host='0.0.0.0', port=18200)
```

## 直观使用

```python
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
tools = store.for_store().list_tools()
# store.for_store().use_tool(tools[0].name, {"query":'hi!'})
```



## LangChain集成示例

将mcpstore工具简单的集成到langchain Agent中，这是一个可以直接运行的代码：

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



## 链式调用设计

MCPStore采用链式调用设计，提供清晰的上下文隔离：

- `store.for_store()` - 全局store空间
- `store.for_agent("agent_id")` - 为指定Agent创建隔离空间
## 多 Agent 隔离

为不同职能的 Agent 分配 `专属的工具集`,积极支持A2A协议，支持快速生成agent card。
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


## API接口

提供完整的RESTful API，一行命令启动Web服务：

```bash
pip install mcpstore
mcpstore run api
```

### 主要API接口

```bash
# 服务管理
POST /for_store/add_service          # 添加服务
GET  /for_store/list_services        # 获取服务列表
POST /for_store/delete_service       # 删除服务

# 工具操作
GET  /for_store/list_tools           # 获取工具列表
POST /for_store/use_tool             # 执行工具

# 监控统计
GET  /for_store/get_stats            # 系统统计
GET  /for_store/health               # 健康检查
```


## 参与贡献

欢迎社区贡献：

- ⭐ 给项目点Star
- 🐛 提交Issues报告问题
- 🔧 提交Pull Requests贡献代码
- 💬 分享使用经验和最佳实践

## Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=whillhill/mcpstore&type=Date)](https://star-history.com/#whillhill/mcpstore&Date)

</div>

---

**McpStore是一个还在频繁的更新的项目，恳求大家给小星并来指点**

