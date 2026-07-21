# Python 集成

Python 是 MCPStore 的用户门面，底层能力由 Rust 提供。适合把 MCP 服务管理和工具发现嵌入 Python 项目。

## 初始化

```python
from mcpstore import MCPStore

store = MCPStore.setup_store(mcpjson_path="./mcp.json")
```

## 添加并等待服务

```python
store.for_store().add_service(
    {
        "mcpServers": {
            "mcpstore-wiki": {"url": "https://example.com/mcp"}
        }
    }
)
store.for_store().wait_service("mcpstore-wiki")
```

## 查看和调用能力

```python
services = store.for_store().list_services()
tools = store.for_store().list_tools()
```

Python 页面按功能与 Rust 能力对应；Python 特有的是门面调用方式，不是另一套运行时。
