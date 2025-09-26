<div align="center">


# McpStore

One-stop open-source high-quality MCP service management tool, making it easy for AI Agents to use various tools

![GitHub stars](https://img.shields.io/github/stars/whillhill/mcpstore) ![GitHub forks](https://img.shields.io/github/forks/whillhill/mcpstore) ![GitHub issues](https://img.shields.io/github/issues/whillhill/mcpstore) ![GitHub license](https://img.shields.io/github/license/whillhill/mcpstore) ![PyPI version](https://img.shields.io/pypi/v/mcpstore) ![Python versions](https://img.shields.io/pypi/pyversions/mcpstore) ![PyPI downloads](https://img.shields.io/pypi/dm/mcpstore?label=downloads)

English | [简体中文](README_zh.md)

🚀 [Live Demo](https://mcpstore.wiki/web_demo/dashboard) | 📖 [Documentation](https://doc.mcpstore.wiki/) | 🎯 [Quick Start](#quick-start)

</div>

## Quick Start

### Installation
```bash
mkdir mcp && cd mcp
python -m venv ./
source bin/activate
pip install aiohttp psutil mcpstore
```

### Online Experience

Open-source Vue frontend interface, supporting intuitive MCP service management through SDK or API

![image-20250721212359929](http://www.text2mcp.com/img/image-20250721212359929.png)

Quick start backend service:

```python
from mcpstore import MCPStore
prod_store = MCPStore.setup_store()
prod_store.start_api_server(host='0.0.0.0', port=18200)
```

## Intuitive Usage

```python
store = MCPStore.setup_store()
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
tools = store.for_store().list_tools()
# store.for_store().use_tool(tools[0].name, {"query":'hi!'})
```

## LangChain Integration Example

Simple integration of mcpstore tools into LangChain Agent, here's a ready-to-run code:

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
    ("system", "You are an assistant, respond with emojis"),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}"),
])
agent = create_tool_calling_agent(llm, tools, prompt)
agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
# ===
query = "What's the weather like in Beijing?"
print(f"\n   🤔: {query}")
response = agent_executor.invoke({"input": query})
print(f"   🤖 : {response['output']}")
```

![image-20250721212658085](http://www.text2mcp.com/img/image-20250721212658085.png)

## Chain Call Design

MCPStore adopts chain call design, providing clear context isolation:

- `store.for_store()` - Global store space
- `store.for_agent("agent_id")` - Create isolated space for specified Agent

## Multi-Agent Isolation

Assign dedicated toolsets for different functional Agents, actively supporting A2A protocol and quick agent card generation.

```python
# Initialize Store
store = MCPStore.setup_store()

# Assign dedicated Wiki tools for "Knowledge Management Agent"
# This operation is performed in the private context of "knowledge" agent
agent_id1 = "my-knowledge-agent"
knowledge_agent_context = store.for_agent(agent_id1).add_service(
    {"name": "mcpstore-wiki", "url": "http://mcpstore.wiki/mcp"}
)

# Assign dedicated development tools for "Development Support Agent"
# This operation is performed in the private context of "development" agent
agent_id2 = "my-development-agent"
dev_agent_context = store.for_agent(agent_id2).add_service(
    {"name": "mcpstore-demo", "url": "http://mcpstore.wiki/mcp"}
)

# Each Agent's toolset is completely isolated without interference
knowledge_tools = store.for_agent(agent_id1).list_tools()
dev_tools = store.for_agent(agent_id2).list_tools()
```

Intuitively, you can use almost all functions through `store.for_store()` and `store.for_agent("agent_id")` ✨


## API Interface

Provides complete RESTful API, start web service with one command:

```bash
pip install mcpstore
mcpstore run api
```

### Main API Endpoints

```bash
# Service Management
POST /for_store/add_service          # Add service
GET  /for_store/list_services        # Get service list
POST /for_store/delete_service       # Delete service

# Tool Operations
GET  /for_store/list_tools           # Get tool list
POST /for_store/use_tool             # Execute tool

# Monitoring & Statistics
GET  /for_store/get_stats            # System statistics
GET  /for_store/health               # Health check
```

## Contributing

Welcome community contributions:

- ⭐ Star the project
- 🐛 Submit Issues to report problems
- 🔧 Submit Pull Requests to contribute code
- 💬 Share usage experiences and best practices

## Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=whillhill/mcpstore&type=Date)](https://star-history.com/#whillhill/mcpstore&Date)

</div>

---

**McpStore is a project under frequent updates, we humbly ask for your stars and guidance**
