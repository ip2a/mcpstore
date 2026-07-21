# Python 集成

Python 是 MCPStore 的用户门面，底层能力通过 Rust 提供。

## 最小流程

```python
# 具体 API 以当前 Python 包版本为准
# 1. 创建或连接 MCPStore
# 2. 添加或读取 MCP 服务
# 3. 查询服务状态
# 4. 发现或调用服务能力
```

Python 文档与 Rust 文档按功能对应，避免维护两套不同的概念说明。
