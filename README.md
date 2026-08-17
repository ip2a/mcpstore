

<div align="center">

<table align="center">
  <tr>
    <td>
<pre>
███    ███  ██████  ███████  ██████  ████████  ██████  ██████  ███████
████  ████ ██      ██    ██ ██          ██    ██    ██ ██   ██ ██
██ ████ ██ ██      ███████  ██████      ██    ██    ██ ██████  █████
██  ██  ██ ██      ██           ██      ██    ██    ██ ██  ██  ██
██      ██  ██████ ██      ██████       ██     ██████  ██   ██ ███████
</pre>
    </td>
  </tr>
</table>

---

![GitHub stars](https://img.shields.io/github/stars/ip2a/mcpstore) ![GitHub forks](https://img.shields.io/github/forks/ip2a/mcpstore) ![GitHub license](https://img.shields.io/github/license/ip2a/mcpstore)  ![Python versions](https://img.shields.io/pypi/pyversions/mcpstore)


[English](README_en.md) | [简体中文](README_zh.md)


[文档](https://ip2a.github.io/mcpstore/) | [快速使用](#简单示例)

</div>

### mcpstore 是什么？

mcpstore 是一个基于 Rust 构建的 MCP 管理平台，覆盖 MCP 服务的配置、运行、调用与生命周期管理，并提供可复用的 SDK、命令行工具（CLI）以及 App/Web 应用等多种使用方式。

### 快速开始

#### SDK

##### Python（PyPI Lib）

```bash
pip install mcpstore
# 或：uv add mcpstore
```

##### Rust Lib

```bash
cargo add mcpstore
```

#### CLI

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/ip2a/mcpstore/main/install.sh | bash

# 或使用 npm（macOS / Linux / Windows）
npm install -g mcpstore
```

#### App

从 [GitHub Releases](https://github.com/ip2a/mcpstore/releases) 下载发行版。

### App 功能

#### 会话管理

统一查看和管理 MCP 会话，掌握当前连接状态与运行情况。

> 截图：`docs/assets/images/app-sessions.png`

#### 会话转移

在不同 Agent 或工作区之间转移会话，保持上下文连续。

> 截图：`docs/assets/images/app-session-transfer.png`

#### 技能管理

集中管理可用工具和技能，按需配置 Agent 的能力范围。

> 截图：`docs/assets/images/app-skills.png`

#### Agent 管理

创建和管理不同 Agent，为每个 Agent 配置独立的服务与工具。

> 截图：`docs/assets/images/app-agents.png`

### CLI 使用

通过 CLI 管理 MCP 服务、查看运行状态，并为 Agent 提供命令行工作流。

### SDK 使用

通过 Python SDK 或 Rust Lib 将 mcpstore 集成到自己的应用中。

### 简单示例

初始化 `store`：

```python
from mcpstore import MCPStore

store = MCPStore.setup_store()
```

通过 `store.for_store()` 管理全局作用域内的 MCP 服务和工具。

#### 添加第一个服务

```python
store.for_store().add_service({
    "mcpServers": {
        "mcpstore_wiki": {
            "url": "https://example.com/mcp"
        }
    }
}).wait_service("mcpstore_wiki")
```

`add_service` 接受 MCP 服务配置；`wait_service` 等待指定服务就绪。

#### 转换为 LangChain 工具

```python
tools = store.for_store().for_langchain().list_tools()
print("loaded langchain tools:", len(tools))
```

适配器从 `store.for_store()` 读取工具，并转换为对应框架使用的对象。

##### 框架适配

| 框架 | 获取工具 |
| --- | --- |
| LangChain | `tools = store.for_store().for_langchain().list_tools()` |
| LangGraph | `tools = store.for_store().for_langgraph().list_tools()` |
| OpenAI | `tools = store.for_store().for_openai().list_tools()` |
| AutoGen | `tools = store.for_store().for_autogen().list_tools()` |
| CrewAI | `tools = store.for_store().for_crewai().list_tools()` |
| LlamaIndex | `tools = store.for_store().for_llamaindex().list_tools()` |
| Semantic Kernel | `tools = store.for_store().for_semantic_kernel().list_tools()` |

#### 在 LangChain 中使用

```python
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    temperature=0,
    model="your-model",
    api_key="sk-*****",
    base_url="https://api.xxx.com",
)
agent = create_agent(model=llm, tools=tools, system_prompt="你是一个助手")
events = agent.invoke({
    "messages": [{"role": "user", "content": "mcpstore 怎么添加服务？"}]
})
print(events)
```

这里的 `tools` 由 `store.for_store()` 提供。


#### 为 Agent 分组

使用 `for_agent(agent_id)` 为不同 Agent 建立独立作用域：

```python
store.for_agent("agent1").add_service({
    "name": "mcpstore_wiki",
    "url": "https://example.com/mcp",
})

store.for_agent("agent2").add_service({
    "name": "gitodo",
    "command": "uvx",
    "args": ["gitodo"],
})

agent1_tools = store.for_agent("agent1").list_tools()
agent2_tools = store.for_agent("agent2").list_tools()
```

`store.for_agent(agent_id)` 与 `store.for_store()` 提供相同的操作，服务和工具按 Agent 作用域隔离。

#### Rust API 与 MCP Server

当前推荐直接使用 Rust CLI 暴露服务，而不是再依赖历史 Python hub 接口：

```bash
# 启动 Rust HTTP API
mcpstore api --config-path ./mcp.json --host 127.0.0.1 --port 1820

# 以 stdio 启动 Rust MCP Server
mcpstore mcp --config-path ./mcp.json

# 以 streamable-http 启动 Rust MCP Server
mcpstore mcp --config-path ./mcp.json --transport streamable-http --host 127.0.0.1 --port 1830 --path /mcp
```

Python SDK 不再启动嵌入式 API server；需要对外提供服务时，请使用 Rust CLI。


#### 常用接口

以下示例使用完整调用链：

| 动作 | 示例 |
| --- | --- |
| 添加服务 | `store.for_store().add_service(config)` |
| 定位服务 | `store.for_store().find_service("service_name")` |
| 查看服务信息 | `store.for_store().find_service("service_name").info()` |
| 查看服务状态 | `store.for_store().find_service("service_name").state()` |
| 更新服务 | `store.for_store().update_service("service_name", new_config)` |
| 增量更新 | `store.for_store().patch_service("service_name", updates)` |
| 等待就绪 | `store.for_store().wait_service("service_name", timeout=30)` |
| 重启服务 | `store.for_store().restart_service("service_name")` |
| 断开服务 | `store.for_store().disconnect_service("service_name")` |
| 删除服务 | `store.for_store().remove_service(service_name="service_name")` |
| 列出服务 | `store.for_store().list_services()` |
| 列出工具 | `store.for_store().list_tools()` |
| 列出当前作用域资源 | `store.for_store().list_resources()` |
| 列出当前作用域资源模板 | `store.for_store().list_resource_templates()` |
| 读取指定服务资源 | `store.for_store().find_service("service_name").read_resource("resource://uri")` |
| 列出当前作用域 Prompts | `store.for_store().list_prompts()` |
| 获取指定服务 Prompt | `store.for_store().find_service("service_name").get_prompt("prompt_name", {"k": "v"})` |
| 调用工具 | `store.for_store().find_tool("tool_name").call({"k": "v"})` |
| 查看配置 | `store.for_store().show_config()` |
| 列出 Agent | `store.list_agents()` |

#### 数据源共享

可以使用 Redis 等 KV 后端，在多个进程或实例之间共享服务与工具数据。

##### Redis 示例

```python
from mcpstore import MCPStore
from mcpstore.config import RedisConfig

redis_config = RedisConfig(
    host="127.0.0.1",
    port=6379,
    password=None,
    namespace="demo_namespace",
)
store = MCPStore.setup_store(source=redis_config)
```

使用相同后端和 `namespace` 的实例可以共享数据。若当前进程只使用共享数据源、不维护本地服务实例，可设置 `mode="data_plane"`：

```python
from mcpstore import MCPStore
from mcpstore.config import RedisConfig

redis_config = RedisConfig(
    host="127.0.0.1",
    port=6379,
    password=None,
    namespace="demo_namespace",
)
store = MCPStore.setup_store(source=redis_config, mode="data_plane")
services = store.for_store().list_services()
```

### Docker 部署

仓库提供 Docker 配置，可用于本地试用和部署。


## Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=ip2a/mcpstore&type=Date)](https://star-history.com/#ip2a/mcpstore&Date)

</div>

---

欢迎通过 Issues 提交问题与建议。
