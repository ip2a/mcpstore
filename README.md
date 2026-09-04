<h4 align="right">
  <strong>English</strong> | <a href="README_zh.md">简体中文</a>
</h4>

<h1 align="center">mcpstore</h1>

<p align="center">
  <strong>MCP management built with Rust</strong><br>
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

## Why mcpstore

mcpstore is built with Rust for high performance and low overhead. Its architecture mirrors Kubernetes-style control plane / data plane: the control plane handles orchestration, while the data plane focuses on invocation. On resource-constrained hosts you can attach only the data plane and call already-shared MCP services with zero local maintenance. SDKs are available for embedding into applications and Agents.

## Quick Start

```bash
npm install -g ip2a@mcpstore
```

or

```bash
curl -fsSL https://raw.githubusercontent.com/ip2a/mcpstore/main/install.sh | bash
```

Prefer code integration? → [Use via SDK](#sdk-usage)



## CLI Usage

Manage MCP services and runtime state from the CLI. See Quick Start above for installation.

Add, connect, restart, or remove a service:

```bash
mcpstore add wiki https://example.com/mcp
mcpstore list
mcpstore connect wiki
mcpstore remove wiki
```

Inspect local config, or open the Web UI:

```bash
mcpstore config show
mcpstore web
```

Expose an HTTP API:

```bash
mcpstore api --host 127.0.0.1 --port 1820
```

## Desktop App (Beta)

The GUI is the control plane: manage MCP services and tools, inspect parameters, and try invocations. Currently in Beta — download from [GitHub Releases](https://github.com/ip2a/mcpstore/releases/latest).

<p align="center">
  <img src="./docs/assets/images/app-tools.png" width="100%" alt="mcpstore tools panel">
</p>


## SDK Usage

Integrate mcpstore into your application via the Python SDK or Rust Lib.

<details>
<summary><strong>Python</strong></summary>

### Install

```bash
pip install mcpstore
```

### Quick Start

Initialize a `store`:

```python
from mcpstore import MCPStore

store = MCPStore.setup_store()
```

Use `store.for_store()` to manage MCP services and tools in the global scope.

#### Add your first service

```python
store.for_store().add_service({
    "mcpServers": {
        "mcpstore_wiki": {
            "url": "https://example.com/mcp"
        }
    }
})
store.for_store().wait_service("mcpstore_wiki")
```


#### Convert to LangChain tools and use them

```python
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

tools = store.for_store().for_langchain().list_tools()

llm = ChatOpenAI(
    temperature=0,
    model="your-model",
    api_key="sk-*****",
    base_url="https://api.xxx.com",
)
agent = create_agent(model=llm, tools=tools, system_prompt="You are an assistant")
events = agent.invoke({
    "messages": [{"role": "user", "content": "How do I add a service in mcpstore?"}]
})
print(events)
```

| Framework | Get tools |
| --- | --- |
| LangChain | `tools = store.for_store().for_langchain().list_tools()` |
| LangGraph | `tools = store.for_store().for_langgraph().list_tools()` |
| OpenAI | `tools = store.for_store().for_openai().list_tools()` |
| AutoGen | `tools = store.for_store().for_autogen().list_tools()` |
| CrewAI | `tools = store.for_store().for_crewai().list_tools()` |
| LlamaIndex | `tools = store.for_store().for_llamaindex().list_tools()` |


#### Group by Agent

Use `for_agent(agent_id)` to create an independent scope per Agent:

```python
store.for_agent("agent1").add_service({
    "name": "mcpstore_wiki",
    "url": "https://example.com/mcp",
})

store.for_agent("agent2").add_service({
    "name": "github",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"]
    },
})

agent1_tools = store.for_agent("agent1").list_tools()
agent2_tools = store.for_agent("agent2").list_tools()
```

</details>

<details>
<summary><strong>Rust</strong></summary>

### Install

```bash
cargo add mcpstore
```

### Quick Start

```rust
use std::time::Duration;

use mcpstore::{McpConfig, MCPStore, Result, ServiceTarget};

#[tokio::main]
async fn main() -> Result<()> {
    let store = MCPStore::setup(None)?;

    store
        .for_store()
        .add_service(McpConfig::from_json_str(
            r#"{"name":"mcpstore_wiki","url":"https://example.com/mcp"}"#,
        )?)
        .await?;

    store
        .for_store()
        .wait_service(
            ServiceTarget::ServiceName("mcpstore_wiki"),
            Duration::from_secs(30),
        )
        .await?;

    let tools = store.for_store().list_tools().await?;
    println!("loaded tools: {}", tools.len());
    Ok(())
}
```


</details>

<details>
<summary><strong>Common APIs</strong></summary>

| Action | Example |
| --- | --- |
| Add service | `store.for_store().add_service(config)` |
| Get service info | `store.for_store().find_service("service_name").info()` |
| Update service | `store.for_store().update_service("service_name", new_config)` |
| Restart service | `store.for_store().restart_service("service_name")` |
| Disconnect service | `store.for_store().disconnect_service("service_name")` |
| Remove service | `store.for_store().remove_service(service_name="service_name")` |
| List services | `store.for_store().list_services()` |
| List tools | `store.for_store().list_tools()` |
| List prompts in current scope | `store.for_store().list_prompts()` |
| Show config | `store.for_store().show_config()` |
| List agents | `store.list_agents()` |

</details>

## Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=ip2a/mcpstore&type=Date)](https://star-history.com/#ip2a/mcpstore&Date)

</div>

---

Issues and suggestions are welcome via GitHub Issues.
