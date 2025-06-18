# 🚀 MCPStore: 企业级MCP工具链管理解决方案

MCPStore 是一个专为解决大语言模型（LLM）应用在生产环境中实际痛点而设计的企业级MCP（Model Context Protocol）工具管理库。它致力于简化AI Agent的工具集成、服务管理和系统监控流程，帮助开发者构建更强大、更可靠的AI应用。

## 1. 项目背景：应对AI Agent开发的挑战

在构建复杂的AI Agent系统时，开发者普遍面临以下挑战：

* **工具集成成本高昂**：为Agent引入新工具通常需要编写大量重复的“胶水代码”，流程繁琐且效率低下。
* **服务管理与维护复杂**：对多个MCP服务的生命周期（注册、发现、更新、注销）进行有效管理，并确保其高可用性，是一项艰巨的任务。
* **服务稳定性保障困难**：网络波动或服务异常可能导致连接中断，缺乏有效的自动重连和健康检查机制会严重影响Agent的稳定性。
* **生态集成壁垒**：将不同来源、不同协议的MCP工具无缝集成到如LangChain、LlamaIndex等主流AI框架中，存在较高的技术门槛。

MCPStore正是为应对这些挑战而生，旨在提供一个统一、高效、可靠的解决方案。

## 2. 核心理念：三行代码，化繁为简

MCPStore的核心设计理念是将复杂性封装，提供极致简洁的用户体验。传统方式需要数十行代码才能完成的工具集成工作，使用MCPStore仅需三行即可实现。

```python
# 引入MCPStore库
from mcpstore import MCPStore

# 步骤1: 初始化Store，这是管理所有MCP服务的核心入口
store = MCPStore.setup_store()

# 步骤2: 注册一个外部MCP服务，MCPStore会自动处理连接和工具加载
await store.for_store().add_service({"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"})

# 步骤3: 获取与LangChain完全兼容的工具列表，可直接用于Agent
tools = await store.for_store().for_langchain().list_tools()

# 此刻，您的LangChain Agent已成功集成了mcpstore-wiki提供的所有工具
```

## 3. LangChain 实战：一个完整的可运行示例

下面是一个完整的、可直接运行的示例，展示了如何将MCPStore获取的工具无缝集成到标准的LangChain Agent中。

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
    一个完整的演示函数，展示如何：
    1. 使用 MCPStore 加载工具。
    2. 配置一个标准的 LangChain Agent。
    3. 将 MCPStore 工具集成到 Agent 中并执行。
    """
    # 步骤 1: 使用 MCPStore 的核心三行代码获取工具
    store = MCPStore.setup_store()
    context = await store.for_store().add_service({"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"})
    mcp_tools = await context.for_langchain().list_tools()

    # 步骤 2: 配置一个强大的语言模型
    # 请注意：您需要将 "YOUR_DEEPSEEK_API_KEY" 替换为您自己的有效API密钥。
    llm = ChatOpenAI(
        temperature=0,
        model="deepseek-chat",
        openai_api_key="YOUR_DEEPSEEK_API_KEY",
        openai_api_base="[https://api.deepseek.com](https://api.deepseek.com)"
    )

    # 步骤 3: 构建 Agent 的思考链 (Chain)
    # 这是一个标准的 LangChain Agent 设置，用于处理输入、调用工具和格式化中间步骤。
    prompt = ChatPromptTemplate.from_messages([
        ("system", "你是一个强大的助手。"),
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

    # 步骤 4: 执行 Agent 并获取结果
    test_question = "北京今天的天气"
    print(f"🤔 提问: {test_question}")

    response = await agent_executor.ainvoke({"input": test_question})
    print(f"\n🎯 Agent回答:")
    print(f"{response['output']}")


if __name__ == "__main__":
    # 使用 asyncio 运行异步主函数
    asyncio.run(main())
```

## 4. 强大的服务注册 `add_service`

MCPStore 提供了高度灵活的 `add_service` 方法来集成不同来源和类型的工具服务。

### 服务注册方式

`add_service` 支持多种参数格式，以适应不同的使用场景：

* **从配置文件加载**：
    不传递任何参数，`add_service` 会自动查找并加载项目根目录下的 `mcp.json` 文件，该文件兼容主流格式。
    ```python
    # 自动加载 mcp.json
    await store.for_store().add_service()
    ```

* **通过URL注册**：
    最常见的方式，直接提供服务的名称和URL。MCPStore会自动推断传输协议。
    ```python
    # 通过网络地址添加服务
    await store.for_store().add_service({
       "name": "weather",
       "url": "[https://weather-api.example.com/mcp](https://weather-api.example.com/mcp)",
       "transport": "streamable-http" # transport 可选，会自动推断
    })
    ```

* **通过本地命令启动**：
    对于本地脚本或可执行文件提供的服务，可以直接指定启动命令。
    ```python
    # 将本地Python脚本作为服务启动
    await store.for_store().add_service({
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
    await store.for_store().add_service({
       "mcpServers": {
           "weather": {
               "url": "[https://weather-api.example.com/mcp](https://weather-api.example.com/mcp)"
           }
       }
    })
    ```
所有通过 `add_service` 添加的服务，其配置都会被统一管理，并可选择持久化到 `mcp.json` 文件中。

## 5. 全面的RESTful API

除了作为Python库使用，MCPStore还提供了一套完备的RESTful API，让您可以将MCP工具管理能力无缝集成到任何后端服务或管理平台中。

一行命令即可启动完整的Web服务：
```bash
pip install mcpstore
mcpstore run api
```
启动后，您将立即获得 **38个** 专业API接口！

### 📡 完整的API生态

#### Store级别API（17个接口）
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

#### Agent级别API（17个接口）
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

#### 通用API（1个接口）
```bash
GET  /services/{name}                # 跨上下文服务查询
```

## 6. 核心设计：链式调用与上下文管理

MCPStore采用富有表现力的链式API设计，使代码逻辑更加清晰、易读。同时，通过**上下文隔离（Context Isolation）**机制，为不同的Agent或全局Store提供独立且安全的服务管理空间。

* `store.for_store()`：进入全局上下文，在此处管理的服务和工具对所有Agent可见。
* `store.for_agent("agent_id")`：为指定ID的Agent创建一个隔离的私有上下文。每个Agent的工具集互不干扰，是实现多租户和复杂Agent系统的关键。

### 场景：构建多Agent隔离的复杂系统

以下代码演示了如何利用上下文隔离，为不同职能的Agent分配专属的工具集。
```python
# 初始化Store
store = MCPStore.setup_store()

# 为“知识管理Agent”分配专用的Wiki工具
# 该操作在"knowledge" agent的私有上下文中进行
agent_id1 = "my-knowledge-agent"
knowledge_agent_context = await store.for_agent(agent_id1).add_service(
    {"name": "mcpstore-wiki", "url": "[http://59.110.160.18:21923/mcp](http://59.110.160.18:21923/mcp)"}
)

# 为“开发支持Agent”分配专用的开发工具
# 该操作在"development" agent的私有上下文中进行
agent_id2 = "my-development-agent"
dev_agent_context = await store.for_agent(agent_id2).add_service(
    {"name": "mcpstore-demo", "url": "[http://59.110.160.18:21924/mcp](http://59.110.160.18:21924/mcp)"}
)

# 各Agent的工具集完全隔离，互不影响
knowledge_tools = await store.for_agent(agent_id2).list_tools()
dev_tools = await store.for_agent(agent_id2).list_tools()
```

## 7. 核心特性
### 7.1. 统一的服务管理
提供强大的服务生命周期管理能力，支持多种服务注册方式，并内置健康检查机制。
### 7.2. 无缝的框架集成
设计时充分考虑了与主流AI框架的兼容性，可以轻松地将MCP工具生态集成到现有工作流中。
### 7.3. 企业级的监控与可靠性
内置了生产级的监控系统，具备服务自动恢复能力，保障系统在复杂环境下的高可用性。

* **自动健康检查**：周期性地检测所有服务的状态。
* **智能重连机制**：在服务断连后，自动尝试重连，并支持指数退避策略，避免冲击服务。
* **动态配置热更新**：通过API实时调整监控参数，无需重启服务。

## 8. 安装与快速上手
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



## 9. 为什么选择MCPStore？

* **极致的开发效率**：将复杂的工具集成流程缩减至几行代码，显著提升开发迭代速度。
* **生产级的稳定与可靠**：内置健康检查、智能重连和资源管理策略，确保在高负载和复杂网络环境下服务的稳定运行。
* **体系化的解决方案**：提供从Python库到RESTful API，再到监控系统的端到端工具链管理方案。
* **强大的生态兼容性**：无缝对接LangChain等主流框架，并支持多种MCP服务协议。
* **灵活的多租户架构**：通过Agent级别的上下文隔离，轻松支持复杂的多Agent应用场景。


## 10. 开发者文档与资源

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


## 10. 参与贡献

MCPStore是一个开源项目，我们欢迎社区的任何形式的贡献：

* ⭐ 如果项目对您有帮助，请在 **GitHub** 上给我们一个Star。
* 🐛 通过 **Issues** 提交错误报告或功能建议。
* 🔧 通过 **Pull Requests** 贡献您的代码。
* 💬 加入社区，分享您的使用经验和最佳实践。

---

**MCPStore：让MCP工具管理变得简单而强大。**
