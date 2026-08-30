<h4 align="right">
  <strong>English</strong> | <a href="README_zh.md">简体中文</a>
</h4>

<h1 align="center">mcpstore</h1>

<p align="center">
  <strong>MCP management, built with Rust.</strong><br>
</p>

<p align="center">
  <a href="https://github.com/ip2a/mcpstore/releases/latest"><img src="https://img.shields.io/github/v/release/ip2a/mcpstore?color=222222" alt="GitHub Release"></a>
  <a href="https://github.com/ip2a/mcpstore/stargazers"><img src="https://img.shields.io/github/stars/ip2a/mcpstore?color=222222" alt="GitHub stars"></a>
  <a href="https://github.com/ip2a/mcpstore/network/members"><img src="https://img.shields.io/github/forks/ip2a/mcpstore?color=222222" alt="GitHub forks"></a>
  <img src="https://img.shields.io/badge/built_with-Rust-dea584?logo=rust&logoColor=white" alt="Rust">
</p>

<p align="center">
  <a href="https://github.com/ip2a/mcpstore/releases/latest">
    <img src="https://img.shields.io/badge/download-macOS-222222?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS">
  </a>
  <a href="https://github.com/ip2a/mcpstore/releases/latest">
    <img src="https://img.shields.io/badge/download-Windows-0078d7?style=for-the-badge&logo=windows&logoColor=white" alt="Download for Windows">
  </a>
  <a href="https://github.com/ip2a/mcpstore/releases/latest">
    <img src="https://img.shields.io/badge/download-Linux-fcc624?style=for-the-badge&logo=linux&logoColor=black" alt="Download for Linux">
  </a>
</p>

## Quick Start

### SDK

#### Python (PyPI)

```bash
pip install mcpstore
```

#### Rust Lib

```bash
cargo add mcpstore
```

### CLI

```bash
curl -fsSL https://raw.githubusercontent.com/ip2a/mcpstore/main/install.sh | bash
```

```bash
npm install -g mcpstore
```

## App Features

### Services

View and manage all MCP services in one place: connection state, health, and tool / resource / prompt counts at a glance, with search, sorting, and filtering.

<p align="center">
  <img src="./docs/assets/images/app-services.png" width="100%" alt="mcpstore service list">
</p>

### Tools

Browse every tool exposed by your services, inspect parameters and responses, and run test calls right from the UI.

<p align="center">
  <img src="./docs/assets/images/app-tools.png" width="100%" alt="mcpstore tool registry">
</p>

### Scope

Partition service and tool visibility per agent, with independent store and agent scopes.

<p align="center">
  <img src="./docs/assets/images/app-agents.png" width="100%" alt="mcpstore agent workspace">
</p>

### Cache

Inspect namespaces, collections, and request metrics in the KV store to understand data distribution and hit rates.

<p align="center">
  <img src="./docs/assets/images/app-cache.png" width="100%" alt="mcpstore cache storage">
</p>

## CLI Usage

Manage MCP services, inspect runtime status, and power command-line workflows for agents through the CLI.

## SDK Usage

Integrate mcpstore into your own applications via the Python SDK or the Rust lib.

## Quick Example

Initialize the `store`:

```python
from mcpstore import MCPStore

store = MCPStore.setup_store()
```

Use `store.for_store()` to manage MCP services and tools in the global scope.

### Add Your First Service

```python
store.for_store().add_service({
    "mcpServers": {
        "mcpstore_wiki": {
            "url": "https://example.com/mcp"
        }
    }
}).wait_service("mcpstore_wiki")
```

`add_service` accepts an MCP service config; `wait_service` blocks until the given service is ready.

### Convert to LangChain Tools

```python
tools = store.for_store().for_langchain().list_tools()
print("loaded langchain tools:", len(tools))
```

The adapter reads tools from `store.for_store()` and converts them into objects for the target framework.

#### Framework Adapters

| Framework | Get Tools |
| --- | --- |
| LangChain | `tools = store.for_store().for_langchain().list_tools()` |
| LangGraph | `tools = store.for_store().for_langgraph().list_tools()` |
| OpenAI | `tools = store.for_store().for_openai().list_tools()` |
| AutoGen | `tools = store.for_store().for_autogen().list_tools()` |
| CrewAI | `tools = store.for_store().for_crewai().list_tools()` |
| LlamaIndex | `tools = store.for_store().for_llamaindex().list_tools()` |
| Semantic Kernel | `tools = store.for_store().for_semantic_kernel().list_tools()` |

### Use in LangChain

```python
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    temperature=0,
    model="your-model",
    api_key="sk-*****",
    base_url="https://api.xxx.com",
)
agent = create_agent(model=llm, tools=tools, system_prompt="You are a helpful assistant")
events = agent.invoke({
    "messages": [{"role": "user", "content": "How do I add a service in mcpstore?"}]
})
print(events)
```

Here `tools` is provided by `store.for_store()`.


### Group by Agent

Use `for_agent(agent_id)` to create an isolated scope per agent:

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

`store.for_agent(agent_id)` exposes the same operations as `store.for_store()`; services and tools are isolated per agent scope.

### Rust API and MCP Server

The recommended way to expose services today is the Rust CLI directly, instead of the legacy Python hub interface:

```bash
# Start the Rust HTTP API
mcpstore api --config-path ./mcp.json --host 127.0.0.1 --port 1820

# Start the Rust MCP Server over stdio
mcpstore mcp --config-path ./mcp.json

# Start the Rust MCP Server over streamable-http
mcpstore mcp --config-path ./mcp.json --transport streamable-http --host 127.0.0.1 --port 1830 --path /mcp
```

The Python SDK no longer starts an embedded API server; use the Rust CLI when you need to serve externally.


### Common APIs

The examples below use the full call chain:

| Action | Example |
| --- | --- |
| Add service | `store.for_store().add_service(config)` |
| Find service | `store.for_store().find_service("service_name")` |
| Service info | `store.for_store().find_service("service_name").info()` |
| Service state | `store.for_store().find_service("service_name").state()` |
| Update service | `store.for_store().update_service("service_name", new_config)` |
| Patch service | `store.for_store().patch_service("service_name", updates)` |
| Wait until ready | `store.for_store().wait_service("service_name", timeout=30)` |
| Restart service | `store.for_store().restart_service("service_name")` |
| Disconnect service | `store.for_store().disconnect_service("service_name")` |
| Remove service | `store.for_store().remove_service(service_name="service_name")` |
| List services | `store.for_store().list_services()` |
| List tools | `store.for_store().list_tools()` |
| List resources in scope | `store.for_store().list_resources()` |
| List resource templates in scope | `store.for_store().list_resource_templates()` |
| Read a service resource | `store.for_store().find_service("service_name").read_resource("resource://uri")` |
| List prompts in scope | `store.for_store().list_prompts()` |
| Get a service prompt | `store.for_store().find_service("service_name").get_prompt("prompt_name", {"k": "v"})` |
| Call a tool | `store.for_store().find_tool("tool_name").call({"k": "v"})` |
| Show config | `store.for_store().show_config()` |
| List agents | `store.list_agents()` |

### Shared Data Source

Use a KV backend such as Redis to share services and tools across processes or instances.

#### Redis Example

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

Instances using the same backend and `namespace` share data. If a process only consumes the shared data source and maintains no local service instances, set `mode="data_plane"`:

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

## Docker Deployment

The repo ships Docker configuration for local trials and deployments.

## Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=ip2a/mcpstore&type=Date)](https://star-history.com/#ip2a/mcpstore&Date)

</div>

---

Feedback and suggestions are welcome via Issues.
