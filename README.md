

<div align="center">

# McpStore


轻松管理MCP服务的SDK,适配主流AI框架,Agent快速调用MCP服务

![GitHub stars](https://img.shields.io/github/stars/whillhill/mcpstore) ![GitHub forks](https://img.shields.io/github/forks/whillhill/mcpstore) ![GitHub issues](https://img.shields.io/github/issues/whillhill/mcpstore) ![GitHub license](https://img.shields.io/github/license/whillhill/mcpstore) ![PyPI version](https://img.shields.io/pypi/v/mcpstore) ![Python versions](https://img.shields.io/pypi/pyversions/mcpstore) ![PyPI downloads](https://img.shields.io/pypi/dm/mcpstore?label=downloads) ![PyPI - Wheel](https://img.shields.io/pypi/wheel/mcpstore) ![code style: black](https://img.shields.io/badge/code%20style-black-000000.svg) ![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)

[English](README.md) | 简体中文

🚀 [在线体验](https://mcpstore.wiki/web_demo/dashboard) | 📖 [详细文档](https://doc.mcpstore.wiki/) | 🎯 [快速开始](#快速开始)

</div>

### 快速开始

```bash
pip install mcpstore
```


### mcpstore是什么？

用户友好的mcp服务管理sdk，方便快速集成MCP服务，并集成了主流agent框架的适配器，简单几行代码就将MCP服务转为agent框架格式的tools对象


### LangChain 示例

```python
from mcpstore import MCPStore
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
tools = store.for_store().for_langchain().list_tools()
```
到这里我们将一个mcp服务做成了langchain可以直接使用的tools对象 基于上面的代码 我们可以添加下面的代码运行

```python
#需要添加上面的代码块
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
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
同时也适配了多种agent框架，比如langgraph autogen等等，通过for_XXX()方法可以快速获取对应的工具对象


### 缓存/Redis 后端
支持使用 Redis 作为共享缓存后端，用于跨进程/多实例共享服务与工具元数据。

安装额外依赖：

```bash
pip install mcpstore[redis]
```

使用方式（通过 `external_db` 传入缓存模块配置）：

```python
from mcpstore import MCPStore

store = MCPStore.setup_store(
    external_db={
        "cache": {
            "type": "redis",
            "url": "redis://localhost:6379/0",   # 必填
            "password": None,                      # 可选
            "namespace": "demo_namespace"      # 可选；
        }
    }
)
```

## 在线体验

简单开源的Vue，支持通过SDK或API方式直观管理MCP服务

![image-20250721212359929](http://www.text2mcp.com/img/image-20250721212359929.png)

快速启动后端服务：

```python
from mcpstore import MCPStore
prod_store = MCPStore.setup_store()
prod_store.start_api_server(host='0.0.0.0', port=18200)
```

###  你也可以直接调用工具

```python
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
tools = store.for_store().list_tools()
store.for_store().call_tool(tools[0].name, {"query":'hi!'})
```



![image-20250721212658085](http://www.text2mcp.com/img/image-20250721212658085.png)



### MCP服务分组

使用for_agent(agent_id)可以将mcp分组 方便不同的agent获取精确的有限上下文工具集
即将支持根据分组 一键生成a2a协议的card

- `store.for_store()` - 全局store空间
- `store.for_agent("agent_id")` - 为指定Agent创建隔离空间

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


### API接口

提供完整的RESTful API，一行命令启动Web服务：

```bash
pip install mcpstore
mcpstore run api
```

### 部分API接口
详细的接口文档看网页

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

### 重要变更（保持与后端最新一致）

- add_service 不再支持 wait 参数；且不支持空参数（必须提供服务配置或 json_file）
- add_service 调用后不等待连接完成，如需等待请在 Agent 级别使用 `/for_agent/{agent_id}/wait_service`，或轮询 `service_status`
- 请求体示例（Store 级别）：

```json
{
  "name": "weather",
  "url": "https://weather.example.com/mcp",
  "transport": "streamable_http",
  "headers": {"Authorization": "Bearer ..."}
}
```

```json
{
  "name": "assistant",
  "command": "python",
  "args": ["./assistant_server.py"],
  "env": {"DEBUG": "true"}
}
```

```json
{
  "mcpServers": {
    "weather": { "url": "https://weather.example.com/mcp" }
  }
}
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

