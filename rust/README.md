# MCPStore Rust Workspace

Rust 工作区承载 MCPStore 的核心运行时、MCP 连接层、CLI 和 Python 绑定。对外仍然是一个 MCPStore 产品；`crates/`、`apps/`、`bindings/` 只是内部职责分层。

## Workspace 结构

| 目录 | 职责 | 对接面 |
|------|------|--------|
| `crates/mcpstore` | 核心库；内部按 `cache`、`config`、`core`、`events`、`registry`、`transport` 分层 | Rust CLI / Python facade |
| `apps/mcpstore` | MCPStore CLI、HTTP API、MCP Server、Web UI、TUI、daemon | `mcpstore`、`mcpstore-tui` |
| `bindings/python` | PyO3 绑定入口 | `mcpstore._rust` |

当前保持单一核心 crate，优先在 crate 内拆清模块职责，再考虑是否需要恢复多 crate 拆分。这样可以保护 Python 绑定和 CLI 的公开接口稳定。

## 构建

```bash
cd rust
cargo check
cargo test
```

## PyO3 打包

`python/pyproject.toml` 使用 `maturin` 构建后端，并加入 `[tool.maturin]`：

- `manifest-path = "../rust/bindings/python/Cargo.toml"`
- `python-source = "src"`
- `module-name = "mcpstore._rust"`

因此 `uv build --wheel` / PEP 517 构建会把 Rust 扩展放进现有 Python 包的 `mcpstore._rust` 模块。

当前 `mcpstore._rust.MCPStore` 已支持 Store 级链路：服务增删改查、连接/断开/重启、工具列表/调用、健康检查、事件历史、cache health、agent scope、配置读取/重置、后端切换和 `shutdown()`。Python 正式入口 `MCPStore.setup_store(...)` 只有一个核心，默认且唯一使用 Rust core。

## 迁移原则

1. **Rust 运行时统一承载核心能力**：MCP 协议连接、缓存、注册表、CLI 和 Python facade 都复用同一套 Rust 能力。
2. **高频数据结构优先迁移**：注册表查找、缓存读写是最大收益点。
3. **内部按组件拆分，对外统一交付**：工作区内部分组件按职责拆分，但对外仍统一为 MCPStore 的 Python 包、CLI 二进制和发布产物。
4. **Rust 是唯一核心**：Python 正式入口直接使用 Rust core；Rust 扩展加载失败时必须显式报错，不保留 Python core 降级路径。
