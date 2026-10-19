# web → GPUI 功能差距审计（2026-10-19）

逐文件对比 `web/src`（169 文件，5 组并行审计）与 `desktop/gpui`（35 个 API 方法 + 现有视图）。
结论：**核心面已替换，但有真实功能缺口**，集中在 4 处：服务表单配置项、auth scope-upgrade、
tools filter=available 写死、settings 的连接/诊断管理。

状态：❌ 缺失（功能不可达）/ ⚠️ 部分（能做但信息/选项少）/ ✅ 已实现 / N.A. 桌面形态不适用

## P0 — 后端能力在 GPUI 完全不可达

| 能力 | web 位置 | GPUI 现状 |
|---|---|---|
| 服务表单缺字段：`env`、`headers`、`cwd`、`oauth`(client-id/client-metadata-url)、`handshake_mode`、`startup_policy`、`restart_policy` | features/services/service-form* | ✅ 2026-10-19 全字段表单（ServiceFormDraft），definition 级 `_mcpstore` 齐全 |
| `POST /services/:name/auth/scope-upgrade` | lib/api/service.ts | ✅ 2026-10-19 auth tab 在 required_scope 存在时显示 Upgrade 按钮 |
| `GET /services/:name/check` 按需健康探测 | features/services | ✅ 2026-10-19 服务行 Check 按钮（探活后刷新列表） |
| tools/list `?filter=` 全部可用（GPUI 写死 `filter=available`，api.rs:480）| features/tools | ⚠️ 2026-10-19 tools 页 available/all 切换；工具策略一览仍缺 |
| 双向 JSON/TOML 导入（粘贴 mcpServers 配置反解析成表单）| service-form* | ✅ 2026-10-19 表单内 Import 区：JSON/TOML 均收，mcpServers 单服务解包回填 |
| settings 连接管理：host 编辑/重命名/删除、HTTPS、延迟探活 | features/settings/connection* | ✅ 2026-10-19 行级 编辑(预填重命名)/删除(活动主机禁删)/http(s) 校验 + 探活延迟芯片（refresh 触发，无 5s 轮询） |
| settings 诊断段：runtime_log 开关/max size/retention | features/settings | ✅ 2026-10-19 enabled/runtime_log 开关按钮 + max_size/retention 输入保存（空=不动现值，旁显 now: 现值） |

## P1 — 部分实现（信息或操作面比 web 少）

| 能力 | web | GPUI |
|---|---|---|
| auth：granted/required scopes、7 态标签、登录轮询、connection_auth_required 引导 | service-panels/auth | ✅ 2026-10-19 authorizing/refreshing 1.5s 轮询（≤5min 自动停）；granted scopes 行已有 |
| MCP hub 对话框：host/port/path 参数、descriptor 展示、5s 轮询 | mcp-hub-dialog | ⚠️ 固定 `?path=/mcp&scope=store`（transport 已随网关改名移除），无 descriptor/轮询；**建议并入网关重设计，勿照旧壳补** |
| 工具详情：annotations/meta/运行时指标块（错误率 p95/p99/last check） | tool-detail | ✅ 2026-10-19 右列 Parameters 卡附 annotations/_meta/output schema JSON（运行时指标在实例页健康块，工具级 web 同样没有） |
| tool-result-view：text/图片/resource 块/structuredContent/isError 徽章 | tool-result-view.tsx | ✅ 2026-10-19 块式渲染（text 错误红框/resource uri+mime+text/空态/error 徽章）；图片为占位卡（base64 data，桌面 img 无 data-URI） |
| 服务详情 3-tab catalog + 右侧 doc/playground；prompt arguments JSON、template annotations/meta JSON | service-panels | ✅ 2026-10-19 prompts 已附 arguments JSON；3-tab 双栏布局不搬（桌面 tab 平铺已够用） |
| 概览：server info/protocol/capabilities badges/instructions | service-panels | ✅ 2026-10-19 Server/Protocol/Capabilities 行 + Instructions 段（mcp 块已随 get_service 返回） |
| 服务列表：排序、4 态状态过滤、scope/agent 过滤对话框、行内 add-scope | services view | ✅ 2026-10-19 phase 循环过滤(all/running/stopped)+名称排序；scope/agent 对话框与行内 add-scope 仍缺（scope 页可达） |
| scopes 页：每 scope 服务下钻、ScopeServiceSheet、每 scope 的 hub/config 入口、可加服务下拉 | agents-view | ⚠️ 只有 registry 列表 + declare/reset |
| settings Overview host 状态卡（探活/延迟/version/store name）、About 链接 | settings | ✅ 2026-10-19 host 行探活延迟芯片 + /health store 名（About 链接无桌面等价） |
| cache：层浏览器/键详情/请求指标网格/复制报告 | features/cache | ⚠️ 整段 JSON dump |
| config `?format=` 指定输出格式 | settings/config | ✅ 2026-10-19 native/claude/codex 循环切换按钮 |
| overrides PUT 只发改动字段；resource override 的 mime_type 字段 | overrides | ✅ 2026-10-19 本就是 patch 语义（enabled+非空字段）；表单补 mime_type 输入 |
| GET /services/:name/state 独立轮询 | — | ⚠️ 用 get_service 全量，等价但重 |
| GET /health 后端状态徽标 + 活动缓存后端展示 | dashboard/settings | ⚠️ 无展示 |
| tools 页 scope/agent/availability 过滤对话框 | tools view | ✅ 2026-10-19 availability(available/all) + scope(all/store/agent) 循环过滤 + 文本搜索（无对话框形态，桌面等价） |

## P2 — 已覆盖或桌面形态不适用

- root 聚合视图 ✅（ScopeView::Root 已进 list_services，api.rs:56）
- 生命周期 connect/disconnect/restart、add/edit/remove scope、agent declare/reset ✅
- tool-policy 设置/清除、tool 调用、resources/templates/prompts 列表与读取 ✅
- overrides 4 类增删/开关 ✅（入口在实例页 Overrides tab）
- cache health/inspect/switch、config 查看/重置（store+agent）、client-config 导入 ✅
- mcp-hub 启停/status、settings 更新、i18n en/zh ✅
- 主题切换：GPUI 多出 web 没有的
- URL 直达路由、浏览器 popup 防拦截、滚动位置保持、键盘导航：N.A.
- Tauri updater：N.A.（非 Tauri shell）
- dashboard hero 统计卡、CLI 命令侧栏预览：外观等价，不做

## 建议补齐顺序

1. 服务表单 7 个缺失字段 + JSON 导入（P0，唯一让配置无法写入的缺口）
2. api.rs 补 `check_service` / `auth_scope_upgrade` / `list_tools` 的 filter 参数
3. settings 连接管理（编辑/删除/延迟）+ 诊断开关
4. auth 状态详情 + 轮询、hub 对话框参数化
5. 其余 P1 按需
