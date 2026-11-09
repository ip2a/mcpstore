# Python API Backend Demo

基于 `mcpstore` PyPI 包搭建的 FastAPI HTTP 后端示例。

## 定位

`mcpstore` 主包本身**不依赖 FastAPI**，只提供 `MCPStore` / `StoreContext` /
`AgentContext` / `Service` / `Tool` 等核心门面，运行时响应统一为普通 `dict`。
本 demo 展示如何**在主包之外**用 FastAPI 把这些能力包装成 HTTP API，
并加上统一的响应信封与请求模型。

请求体直接复用主包模型（如 `MCPServerConfig`），不再各自重新定义。

## 安装

```bash
pip install mcpstore fastapi uvicorn
```

或使用本目录 requirements 文件：

```bash
pip install -r requirements.txt
```

## 运行

```bash
uvicorn app:app --reload --port 18201
```

## 响应格式

所有路由返回统一信封：

```json
{"ok": true, "data": <payload>, "error": null, "message": "ok"}
```

失败时 `ok` 为 `false`，`error` 给出原因（同时附带合适的 HTTP 状态码）。

## 使用

```bash
# 健康检查
curl http://127.0.0.1:18201/health

# 查看 store 配置
curl http://127.0.0.1:18201/config

# 添加服务（标准 mcpServers 结构，复用包内 MCPServerConfig）
curl -X POST http://127.0.0.1:18201/services \
  -H "Content-Type: application/json" \
  -d '{"mcpServers":{"gitodo":{"command":"uvx","args":["gitodo"]}}}'

# 列出服务
curl http://127.0.0.1:18201/services

# 列出工具
curl http://127.0.0.1:18201/tools

# 调用工具（返回 {content, is_error} 作为 data）
curl -X POST http://127.0.0.1:18201/tools/gitodo_get_tasks/call \
  -H "Content-Type: application/json" \
  -d '{"args":{}}'
```
