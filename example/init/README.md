# 初始化测试模块

本模块包含 MCPStore 初始化相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_init_basic.py` | Store 基础初始化 | Store 级别 |
| `test_store_init_redis.py` | Store + Redis 初始化 | Store 级别 |
| `test_agent_init_basic.py` | Agent 基础初始化 | Agent 级别 |
| `test_mixed_init_comparison.py` | Store vs Agent 对比 | 混合模式 |

## 🚀 运行测试

### 运行单个测试

```bash
# Store 基础初始化
python example/init/test_store_init_basic.py

# Store + Redis 初始化
python example/init/test_store_init_redis.py

# Agent 基础初始化
python example/init/test_agent_init_basic.py

# Store vs Agent 对比
python example/init/test_mixed_init_comparison.py
```

### 运行所有初始化测试

```bash
# Windows
for %f in (example\init\test_*.py) do python %f

# Linux/Mac
for f in example/init/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Store 基础初始化
测试 `MCPStore.setup_store()` 的基础功能：
- 无参数初始化
- Debug 模式初始化
- 验证 Context 可用性
- 列出初始服务

### 2. Store + Redis 初始化
测试 Redis 配置的初始化：
- Redis 连接配置
- 命名空间和数据空间
- 故障回退机制
- 服务持久化

### 3. Agent 基础初始化
测试 Agent 级别的初始化：
- 创建单个 Agent Context
- 创建多个 Agent Context
- 验证 Agent 隔离性

### 4. Store vs Agent 对比
对比两种模式的差异：
- 服务空间隔离
- 功能特性对比
- 使用场景建议

## 💡 注意事项

1. **本地 vs 环境导入**
   - 测试文件会优先使用本地 `src/mcpstore`
   - 如果本地不存在，则使用环境中安装的 mcpstore

2. **Redis 测试**
   - 需要本地 Redis 服务运行
   - 如果 Redis 不可用，会显示相应提示
   - MCPStore 会自动回退到内存存储

3. **输出格式**
   - ✅ 表示成功
   - ⚠️ 表示警告
   - ❌ 表示失败
   - 💡 表示提示信息

## 🔗 相关文档

- [快速上手](../../mcpstore_docs/docs/getting-started/quickstart.md)
- [MCPStore 类文档](../../mcpstore_docs/docs/api-reference/mcpstore-class.md)
- [Redis 支持](../../mcpstore_docs/docs/database/redis.md)

