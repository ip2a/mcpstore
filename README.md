[中文](https://github.com/whillhill/mcpstore/blob/main/README_zh.md) | English

# 🚀 MCPStore: Enterprise-Grade MCP Toolchain Management Solution

MCPStore is an enterprise-grade MCP (Model Context Protocol) tool management library designed specifically to address the real-world pain points of Large Language Model (LLM) applications in production environments. It is dedicated to simplifying the process of AI Agent tool integration, service management, and system monitoring, helping developers build more powerful and reliable AI applications.

## 1. Project Background: Addressing the Challenges of AI Agent Development

When building complex AI Agent systems, developers commonly face the following challenges:

* **High Tool Integration Costs**: Introducing new tools to an Agent often requires writing a large amount of repetitive "glue code," making the process cumbersome and inefficient.
* **Complex Service Management and Maintenance**: Effectively managing the lifecycle (registration, discovery, updates, deregistration) of multiple MCP services and ensuring their high availability is a daunting task.
* **Difficulty in Ensuring Service Stability**: Network fluctuations or service abnormalities can lead to connection interruptions. A lack of effective automatic reconnection and health check mechanisms can severely impact the Agent's stability.
* **Ecosystem Integration Barriers**: Seamlessly integrating MCP tools from different sources and with different protocols into mainstream AI frameworks like LangChain and LlamaIndex presents a high technical barrier.

MCPStore was created to address these challenges, aiming to provide a unified, efficient, and reliable solution.

## 2. Core Philosophy: Simplify Complexity with Three Lines of Code

The core design philosophy of MCPStore is to encapsulate complexity and provide an extremely simple user experience. A tool integration task that would traditionally require dozens of lines of code can be accomplished with just three lines using MCPStore.

```python
# Import the MCPStore library
from mcpstore import MCPStore

# Step 1: Initialize the Store, the core entry point for managing all MCP services
store = MCPStore.setup_store()

# Step 2: Register an external MCP service. MCPStore will automatically handle the connection and tool loading
await store.for_store().add_service({"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"})

# Step 3: Get a list of tools fully compatible with LangChain, ready to be used by an Agent
tools = await store.for_store().for_langchain().list_tools()

# At this point, your LangChain Agent has successfully integrated all tools provided by mcpstore-wiki
```

## 3. LangChain in Action: A Complete, Runnable Example

Below is a complete, runnable example that demonstrates how to seamlessly integrate tools fetched by MCPStore into a standard LangChain Agent.

```python
import asyncio

from langchain.agents import AgentExecutor
from langchain.agents.format_scratchpad.openai_tools import (
    format_to_openai_tool_messages,
)
from langchain.agents.output_parsers.openai_tools import OpenAIToolsAgentOutputParser
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
from langchain_openai import ChatOpenAI

from mcpstore import MCPStore


async def main():
    """
    A complete demonstration function showing how to:
    1. Load tools using MCPStore.
    2. Configure a standard LangChain Agent.
    3. Integrate MCPStore tools into the Agent and execute it.
    """
    # Step 1: Get tools with MCPStore's core three lines of code
    store = MCPStore.setup_store()
    context = await store.for_store().add_service({"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"})
    mcp_tools = await context.for_langchain().list_tools()

    # Step 2: Configure a powerful language model
    # Note: You need to replace "YOUR_DEEPSEEK_API_KEY" with your own valid API key.
    llm = ChatOpenAI(
        temperature=0,
        model="deepseek-chat",
        openai_api_key="YOUR_DEEPSEEK_API_KEY",
        openai_api_base="[https://api.deepseek.com](https://api.deepseek.com)"
    )

    # Step 3: Build the Agent's reasoning chain
    # This is a standard LangChain Agent setup for handling input, calling tools, and formatting intermediate steps.
    prompt = ChatPromptTemplate.from_messages([
        ("system", "You are a powerful assistant."),
        ("user", "{input}"),
        MessagesPlaceholder(variable_name="agent_scratchpad"),
    ])

    llm_with_tools = llm.bind_tools(mcp_tools)

    agent_chain = (
        {
            "input": lambda x: x["input"],
            "agent_scratchpad": lambda x: format_to_openai_tool_messages(x["intermediate_steps"]),
        }
        | prompt
        | llm_with_tools
        | OpenAIToolsAgentOutputParser()
    )

    agent_executor = AgentExecutor(agent=agent_chain, tools=mcp_tools, verbose=True)

    # Step 4: Execute the Agent and get the result
    test_question = "What's the weather like in Beijing today?"
    print(f"🤔 Question: {test_question}")

    response = await agent_executor.ainvoke({"input": test_question})
    print(f"\n🎯 Agent Answer:")
    print(f"{response['output']}")


if __name__ == "__main__":
    # Run the async main function using asyncio
    asyncio.run(main())
```

## 4. Powerful Service Registration with `add_service`

MCPStore provides a highly flexible `add_service` method to integrate tool services from different sources and types.

### Service Registration Methods

`add_service` supports multiple parameter formats to suit different use cases:

* **Load from a configuration file**:
  By not passing any arguments, `add_service` will automatically find and load the `mcp.json` file from the project's root directory, which is compatible with mainstream formats.

  ```python
  # Automatically load mcp.json
  await store.for_store().add_service()
  ```

* **Register via URL**:
  The most common method, directly providing the service's name and URL. MCPStore will automatically infer the transport protocol.

  ```python
  # Add a service via its network address
  await store.for_store().add_service({
     "name": "weather",
     "url": "[https://weather-api.example.com/mcp](https://weather-api.example.com/mcp)",
     "transport": "streamable-http" # transport is optional and will be inferred
  })
  ```

* **Start via local command**:
  For services provided by local scripts or executables, you can directly specify the startup command.

  ```python
  # Start a local Python script as a service
  await store.for_store().add_service({
     "name": "assistant",
     "command": "python",
     "args": ["./assistant_server.py"],
     "env": {"DEBUG": "true"}
  })
  ```

* **Register via dictionary configuration**:
  Supports passing a dictionary structure that conforms to the MCPConfig specification directly.

  ```python
  # Add a service using the MCPConfig dictionary format
  await store.for_store().add_service({
     "mcpServers": {
         "weather": {
             "url": "[https://weather-api.example.com/mcp](https://weather-api.example.com/mcp)"
         }
     }
  })
  ```

All services added via `add_service` will have their configurations managed centrally and can optionally be persisted to the `mcp.json` file.

## 5. Comprehensive RESTful API

In addition to being used as a Python library, MCPStore also provides a complete set of RESTful APIs, allowing you to seamlessly integrate MCP tool management capabilities into any backend service or management platform.

A single command starts the full-featured web service:
```bash
pip install mcpstore
mcpstore run api
```

Once started, you will instantly have access to **38** professional API endpoints!

### 📡 A Complete API Ecosystem

#### Store-Level APIs (17 endpoints)
```bash
# Service Management
POST /for_store/add_service          # Add a service
GET  /for_store/list_services        # Get service list
POST /for_store/delete_service       # Delete a service
POST /for_store/update_service       # Update a service
POST /for_store/restart_service      # Restart a service

# Tool Operations
GET  /for_store/list_tools           # Get tool list
POST /for_store/use_tool             # Execute a tool

# Batch Operations
POST /for_store/batch_add_services   # Batch add services
POST /for_store/batch_update_services # Batch update services

# Monitoring & Statistics
GET  /for_store/get_stats            # Get system statistics
GET  /for_store/health               # Health check
```

#### Agent-Level APIs (17 endpoints)
```bash
# Fully correspond to Store-level, supporting multi-tenant isolation
POST /for_agent/{agent_id}/add_service
GET  /for_agent/{agent_id}/list_services
# ... all Store-level functions are supported
```

#### Monitoring System APIs (3 endpoints)
```bash
GET  /monitoring/status              # Get monitoring status
POST /monitoring/config              # Update monitoring configuration
POST /monitoring/restart             # Restart monitoring tasks
```

#### General API (1 endpoint)
```bash
GET  /services/{name}                # Cross-context service query
```

## 6. Core Design: Chainable Calls and Context Management

MCPStore uses an expressive, chainable API design that makes code logic clearer and more readable. At the same time, it provides independent and secure service management spaces for different Agents or the global Store through its **Context Isolation** mechanism.

* `store.for_store()`: Enters the global context. Services and tools managed here are visible to all Agents.
* `store.for_agent("agent_id")`: Creates an isolated, private context for the specified Agent ID. Each Agent's toolset does not interfere with others, which is key to implementing multi-tenancy and complex Agent systems.

### Scenario: Building a Complex System with Isolated Multi-Agents

The following code demonstrates how to use context isolation to assign dedicated toolsets to Agents with different functions.
```python
# Initialize the Store
store = MCPStore.setup_store()

# Assign a dedicated Wiki tool to the "Knowledge Management Agent"
# This operation is performed in the private context of the "knowledge" agent
agent_id1 = "my-knowledge-agent"
knowledge_agent_context = await store.for_agent(agent_id1).add_service(
    {"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"}
)

# Assign dedicated development tools to the "Development Support Agent"
# This operation is performed in the private context of the "development" agent
agent_id2 = "my-development-agent"
dev_agent_context = await store.for_agent(agent_id2).add_service(
    {"name": "mcpstore-demo", "url": "[http://59.110.160.18:21924/mcp](http://59.110.160.18:21924/mcp)"}
)

# The toolsets of each Agent are completely isolated and do not affect each other
knowledge_tools = await store.for_agent(agent_id1).list_tools()
dev_tools = await store.for_agent(agent_id2).list_tools()
```

## 7. Core Features
### 7.1. Unified Service Management
Provides powerful service lifecycle management capabilities, supports multiple service registration methods, and includes a built-in health check mechanism.
### 7.2. Seamless Framework Integration
Designed with compatibility with mainstream AI frameworks in mind, allowing the MCP tool ecosystem to be easily integrated into existing workflows.
### 7.3. Enterprise-Grade Monitoring and Reliability
Includes a production-grade monitoring system with service auto-recovery capabilities, ensuring high availability in complex environments.

* **Automatic Health Checks**: Periodically checks the status of all services.
* **Intelligent Reconnection Mechanism**: Automatically attempts to reconnect after a service disconnection, with support for an exponential backoff strategy to avoid overwhelming the service.
* **Dynamic Configuration Hot-Reload**: Adjust monitoring parameters in real-time via the API without restarting the service.

## 8. Installation and Quick Start
### Installation
```bash
pip install mcpstore
```
### Quick Start
```bash
# Start the full-featured API service
mcpstore run api

# In another terminal, access the monitoring dashboard to get system status
curl http://localhost:18611/monitoring/status

# Test adding an MCP service
curl -X POST http://localhost:18611/for_store/add_service \
  -H "Content-Type: application/json" \
  -d '{"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"}'
```

## 9. Why Choose MCPStore?

* **Extreme Development Efficiency**: Reduces complex tool integration processes to just a few lines of code, significantly accelerating development iterations.
* **Production-Grade Stability and Reliability**: Built-in health checks, intelligent reconnection, and resource management strategies ensure stable service operation under high load and in complex network environments.
* **Systematic Solution**: Provides an end-to-end toolchain management solution, from a Python library to a RESTful API and a monitoring system.
* **Powerful Ecosystem Compatibility**: Seamlessly integrates with mainstream frameworks like LangChain and supports multiple MCP service protocols.
* **Flexible Multi-Tenant Architecture**: Easily supports complex multi-Agent application scenarios through Agent-level context isolation.

## 10. Developer Documentation & Resources

### Detailed API Documentation
We provide exhaustive RESTful API documentation to help developers integrate and debug quickly. The documentation offers comprehensive information for each API endpoint, including:
* **Function Description**: The purpose and business logic of the endpoint.
* **URL and HTTP Method**: Standard request path and method.
* **Request Parameters**: Detailed descriptions, types, and validation rules for input parameters.
* **Response Examples**: Clear examples of success and failure response structures.
* **Curl Call Examples**: Command-line examples that can be copied and run directly.
* **Source Code Traceability**: Links to the backend source file, class, and key functions that implement the API, creating transparency from API to code and greatly facilitating deep debugging and problem-solving.

### Source-Level Developer Documentation (LLM-Friendly)
To support deep customization and secondary development, we also offer a unique source-level reference document. This document not only systematically organizes all the core classes, attributes, and methods in the project but, more importantly, we provide an additional `llm.txt` version optimized for Large Language Models (LLMs).
Developers can directly feed this plain-text document to an AI model, allowing the AI to assist with code comprehension, feature extension, or refactoring, thus achieving true AI-Driven Development.

## 11. Contributing

MCPStore is an open-source project, and we welcome contributions of any kind from the community:

* ⭐ If the project is helpful to you, please give us a Star on **GitHub**.
* 🐛 Submit bug reports or feature suggestions via **Issues**.
* 🔧 Contribute your code via **Pull Requests**.
* 💬 Join the community to share your experiences and best practices.

---

**MCPStore: Making MCP tool management simple and powerful.**
