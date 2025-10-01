# 添加服务测试模块

本模块包含服务注册相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_service_add_local.py` | Store 添加本地服务 | Store 级别 |
| `test_store_service_add_remote.py` | Store 添加远程服务 | Store 级别 |
| `test_store_service_add_json.py` | Store 从 JSON 文件添加 | Store 级别 |
| `test_store_service_add_market.py` | Store 从市场添加服务 | Store 级别 |
| `test_agent_service_add_local.py` | Agent 添加本地服务 | Agent 级别 |
| `test_agent_service_add_remote.py` | Agent 添加远程服务 | Agent 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 添加本地服务
python example/service/add/test_store_service_add_local.py

# Store 添加远程服务
python example/service/add/test_store_service_add_remote.py

# Store 从 JSON 文件添加
python example/service/add/test_store_service_add_json.py

# Store 从市场添加
python example/service/add/test_store_service_add_market.py

# Agent 添加本地服务
python example/service/add/test_agent_service_add_local.py

# Agent 添加远程服务
python example/service/add/test_agent_service_add_remote.py
```

### 运行所有添加服务测试

```bash
# Windows
for %f in (example\service\add\test_*.py) do python %f

# Linux/Mac
for f in example/service/add/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 添加本地服务
测试添加本地命令启动的服务：
- 使用 `command` + `args` 配置
- 等待服务就绪
- 列出服务工具
- 示例：howtocook-mcp

### 2. Store 添加远程服务
测试添加远程 URL 服务：
- 使用 `url` 配置
- 连接远程 MCP 服务
- 测试工具调用
- 示例：weather 服务

### 3. Store 从 JSON 文件添加
测试从配置文件批量添加：
- 创建临时 JSON 文件
- 批量添加多个服务
- 验证服务列表
- 清理临时文件

### 4. Store 从市场添加
测试从 MCPStore 市场安装：
- 使用 `market` 标识
- 自动安装和配置
- 一键集成第三方服务

### 5. Agent 添加本地服务
测试 Agent 级别添加本地服务：
- Agent 独立服务空间
- 验证隔离性
- Store 看不到 Agent 服务

### 6. Agent 添加远程服务
测试 Agent 级别添加远程服务：
- Agent 独立连接
- 多 Agent 隔离验证
- 独立工具调用

## 💡 服务类型对比

### 本地服务
```python
{
    "mcpServers": {
        "service_name": {
            "command": "npx",
            "args": ["-y", "package-name"]
        }
    }
}
```
- ✅ 启动快速
- ✅ 适合开发测试
- ⚠️ 需要本地环境

### 远程服务
```python
{
    "mcpServers": {
        "service_name": {
            "url": "https://example.com/mcp"
        }
    }
}
```
- ✅ 无环境依赖
- ✅ 适合生产环境
- ⚠️ 依赖网络

### 市场服务
```python
{
    "mcpServers": {
        "service_name": {
            "market": "package-id"
        }
    }
}
```
- ✅ 一键安装
- ✅ 自动配置
- ✅ 版本管理

## 🔗 相关文档

- [添加服务文档](../../../mcpstore_docs/docs/services/registration/add-service.md)
- [配置格式说明](../../../mcpstore_docs/docs/services/registration/config-formats.md)
- [完整示例](../../../mcpstore_docs/docs/services/registration/examples.md)

