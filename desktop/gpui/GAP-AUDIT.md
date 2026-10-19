# web → GPUI 功能差距审计（2026-10-19）

逐文件对比 `web/src`（169 文件，5 组并行审计）与 `desktop/gpui`（35 个 API 方法 + 现有视图）。
结论：**核心面已替换，但有真实功能缺口**，集中在 4 处：服务表单配置项、auth scope-upgrade、
tools filter=available 写死、settings 的连接/诊断管理。

状态：❌ 缺失（功能不可达）/ ⚠️ 部分（能做但信息/选项少）/ ✅ 已实现 / N.A. 桌面形态不适用

## P0 — 后端能力在 GPUI 完全不可达

| 能力 | web 位置 | GPUI 现状 |
|---|---|---|
| 服务表单缺字段：`env`、`headers`、`cwd`、`oauth`(client-id/client-metadata-url)、`handshake_mode`、`startup_policy`、`restart_policy` | features/services/service-form* | ❌ 表单只有 command/target/description —— 没有任何路径能写入这些配置 |
| `POST /services/:name/auth/scope-upgrade` | lib/api/service.ts | ❌ api.rs 无此方法；auth 状态 `scope_upgrade_required` 时用户无法操作 |
| `GET /services/:name/check` 按需健康探测 | features/services | ❌ api.rs 无此方法 |
| tools/list `?filter=` 全部可用（GPUI 写死 `filter=available`，api.rs:480）| features/tools | ❌ 被 override 移除的工具在 GPUI **不可见**，也没有工具策略一览 |
| 双向 JSON/TOML 导入（粘贴 mcpServers 配置反解析成表单）| service-form* | ❌ 只能逐字段手填；client-config/import 只覆盖编辑器配置文件场景 |
| settings 连接管理：host 编辑/重命名/删除、HTTPS、延迟探活 | features/settings/connection* | ⚠️ 只有 add/use（api.rs core_base 可换但 UI 无编辑） |
| settings 诊断段：runtime_log 开关/max size/retention | features/settings | ❌ 无 UI（settings JSON 本体可看但没有入口改这段） |

## P1 — 部分实现（信息或操作面比 web 少）

| 能力 | web | GPUI |
|---|---|---|
| auth：granted/required scopes、7 态标签、登录轮询、connection_auth_required 引导 | service-panels/auth | ⚠️ 只有 status 串 + start/refresh/logout，无轮询 |
| MCP hub 对话框：transport/host/port/path/agent_id 参数、descriptor 展示、5s 轮询 | mcp-hub-dialog | ⚠️ 固定 `?transport=stdio&path=/mcp&scope=store`，无 descriptor/轮询 |
| 工具详情：annotations/meta/运行时指标块（错误率 p95/p99/last check） | tool-detail | ⚠️ 只有 schema 字段 + 跑 |
| tool-result-view：text/图片/resource 块/structuredContent/isError 徽章 | tool-result-view.tsx | ⚠️ 整段 pretty JSON |
| 服务详情 3-tab catalog + 右侧 doc/playground；prompt arguments JSON、template annotations/meta JSON | service-panels | ⚠️ tab 平铺、prompts 只有 name/desc |
| 概览：server info/protocol/capabilities badges/instructions | service-panels | ⚠️ 基础字段 |
| 服务列表：排序、4 态状态过滤、scope/agent 过滤对话框、行内 add-scope | services view | ⚠️ 只有文本搜索；add-scope 要绕去 scopes 页 |
| scopes 页：每 scope 服务下钻、ScopeServiceSheet、每 scope 的 hub/config 入口、可加服务下拉 | agents-view | ⚠️ 只有 registry 列表 + declare/reset |
| settings Overview host 状态卡（探活/延迟/version/store name）、About 链接 | settings | ⚠️ GPUI 只显示 version |
| cache：层浏览器/键详情/请求指标网格/复制报告 | features/cache | ⚠️ 整段 JSON dump |
| config `?format=` 指定输出格式 | settings/config | ⚠️ GPUI 固定 native |
| overrides PUT 只发改动字段；resource override 的 mime_type 字段 | overrides | ⚠️ 全量发送；mime_type 缺 |
| GET /services/:name/state 独立轮询 | — | ⚠️ 用 get_service 全量，等价但重 |
| GET /health 后端状态徽标 + 活动缓存后端展示 | dashboard/settings | ⚠️ 无展示 |
| tools 页 scope/agent/availability 过滤对话框 | tools view | ⚠️ 文本搜索 |

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
