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

## Kernel 架构与部署

唯一分层：`MCPStore` 仍是公开门面；内部由 `StoreKernel` 承载 `ControlPlane`、`ExecutionEngine`、`RuntimeState`、`PersistenceRouter`。CLI、KernelHost、HTTP API、Web、MCP transport、TUI 只做输入校验、输出映射和协议编码。

入口策略（daemon-first）：

| 入口 | 执行方式 |
|---|---|
| `mcpstore start` / `mcpstore daemon restart` | 前台/后台启动 daemon：一个进程持有一个 StoreKernel + 四个 HTTP 面 |
| 业务 CLI 命令 | 默认连本机 daemon（不在则后台自动拉起）；`--embedded` 或显式 `--source/--store/--store-config/--namespace/--config-path` 时本进程冷启动 embedded Kernel |
| `mcpstore status` / `mcpstore stop` / 裸 `api` / 裸 `web` | daemon 管理与只读视图命令，不启动进程 |
| `mcpstore config --<key> <value>` | 唯一修改面：经 daemon 热应用（listener 先新后旧）并原子回写 `~/.mcpstore/config.toml`；手动改文件仅在重启 daemon 后生效 |
| `mcp`（聚合 server） | stdio 默认为 thin client：进程形态保留、业务全部转发 daemon（单 kernel）；`--embedded`/显式 store 参数/instance·session 定向模式为本进程 kernel；streamable-http 由 daemon 按 `mcp_aggregate.enabled` 托管 1830 |
| `tui` | 默认连 daemon（自动拉起），与 CLI 共用业务 op；`--embedded` 回退本进程 kernel |

daemon 托管四个 HTTP 面（默认全部 `127.0.0.1`，v1 无认证，远程暴露需自行判断）：Core API `:1820`（全部业务路由，前端 core base 可切远程 daemon）、App API `:1821`（health/meta/settings/config/cache，固定本地）、Web `:1828`（静态资源 + 同源 `/api` → App 面）、聚合 MCP `:1830`（`mcp_aggregate.enabled=true` 且 streamable-http）。单面 bind 失败不致命：该面降级为未运行，kernel socket 不受影响。

KernelHost 是跨进程 typed IPC adapter，不是事实源；业务 op 分发在 `daemon/ops.rs`，daemon socket 与 CLI embedded 两条执行位置共用同一份实现。连接先做 handshake，再收发 JSON-lines：`KernelRequest{request_id, operation, payload, deadline_ms}` 与 `KernelResponse{request_id, event, result, error, kernel_revision}`。协议版本为 1；覆盖 call/tool stream/list/connect/disconnect/restart/check/wait/add/update/scope/config/auth/resource/prompt/completion/task/migrate/events 与 daemon 管理（status/config 热应用/stop）等操作。本机 Unix 默认 `/tmp/mcpstore.sock`（`MCPSTORE_SOCKET`）；Windows 使用 loopback TCP（`MCPSTORE_KERNEL_ENDPOINT`，默认动态端口）；PID 文件为 `/tmp/mcpstore.pid`（`MCPSTORE_PID`）。

daemon 模式下 `call` 的语义边界：elicitation 不可用（headless 语义，等价 `--non-interactive`；交互式 elicitation 需 `--embedded`）；Ctrl-C 断开事件流后 daemon 侧执行继续、结果不自动重放。

OpenKV 是持久化事实源。内存、disk、SQLite、Redis、远程 backend 由 `PersistenceRouter` 统一路由；热迁移只切换 active backend，不重建 `ExecutionEngine`、连接、session 或 `InstanceId`。迁移期写入基线：p50 0.005ms、p95 0.023ms、max 0.060ms。embedded noop tool 基线：p50 15.538ms、p95 17.847ms、64 calls/s；该路径无 HTTP/socket/额外序列化。
