# Python API Backend Demo

基于 `mcpstore` PyPI 包搭建的 FastAPI HTTP 后端示例。

## 定位

`mcpstore` 主包本身**不依赖 FastAPI**，只提供 MCPStore / StoreContext / AgentContext / Service / Tool 等核心门面。
本 demo 展示如何**在主包之外**用 FastAPI 把这些能力包装成 HTTP API。

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
uvicorn app:app --reload --port 8000
```

## 使用

```bash
# 健康检查
curl http://127.0.0.1:8000/health

# 查看 store 配置
curl http://127.0.0.1:8000/config

# 添加服务（name + config 形式）
curl -X POST http://127.0.0.1:8000/services \
  -H "Content-Type: application/json" \
  -d '{"name":"gitodo","config":{"type":"stdio","command":"uvx","args":["gitodo"]}}'

# 列出服务
curl http://127.0.0.1:8000/services

# 列出工具
curl http://127.0.0.1:8000/tools

# 调用工具
curl -X POST http://127.0.0.1:8000/tools/gitodo_get_tasks/call \
  -H "Content-Type: application/json" \
  -d '{"args":{}}'
```
