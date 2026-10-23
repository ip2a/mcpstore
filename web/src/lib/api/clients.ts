// 本模块全部为 app 自有接口（本机编码客户端的发现/接线 + daemon 面网关运行时），
// 固定走 appApi，不随 core 后端切换。
import { appApi, buildQuery } from "./client";
export type DetectedBy = "config_file" | "config_dir" | "command";

export type ClientInventory = {
  kind: string;
  id: string;
  label: string;
  detected_by: DetectedBy | null;
  config_path: string;
  config_exists: boolean;
  mcp_servers: string[] | null;
  unsupported_fields: string[];
  wired: boolean;
  wired_url: string | null;
};

export type GatewayStatus = {
  running: boolean;
  pid: number | null;
  host: string;
  port: number;
  path: string;
  url: string;
  summary: unknown;
};

export type GatewayRequestRecord = {
  seq: number;
  at_unix_ms: number;
  caller: string | null;
  tool: string;
  instance_id: string;
  service_name: string;
  duration_ms: number;
  success: boolean;
};

export type GatewayRequestsPage = {
  running: boolean;
  requests: GatewayRequestRecord[];
  latest_seq: number;
};

export type ScopeRef = { type: "store" } | { type: "agent"; agent_id: string };

export type ExposeConfig = {
  session_state: boolean;
  tool: boolean;
  prompt: boolean;
  resource: boolean;
  openapi: boolean;
  service: boolean;
  cache: boolean;
  search: boolean;
};

export type WiredEntry = {
  client: string;
  path: string;
  name: string;
  url: string;
  applied_at_unix_ms: number;
  // 后端注入的生效值（缺省配置已推导为 Agent{agent_id:client_id} + 全 false）。
  scope: ScopeRef;
  expose: ExposeConfig;
};

/** wire 时可覆盖的作用域/暴露配置；省略即后端默认（专属 scope + 全 false）。 */
export type WireOptions = {
  scope?: "store" | "agent";
  agentId?: string;
  expose?: Partial<ExposeConfig>;
};

export function listClients(): Promise<ClientInventory[]> {
  return appApi<{ clients: ClientInventory[] }>("/clients").then(
    (payload) => payload.clients,
  );
}

/** path/url 省略时由后端发现配置文件并生成 caller URL。 */
export function wireClient(client: string, options?: WireOptions): Promise<WiredEntry> {
  const expose = options?.expose;
  const query = buildQuery({
    client,
    scope: options?.scope,
    agent_id: options?.agentId,
    expose_session_state: expose?.session_state,
    expose_tool: expose?.tool,
    expose_prompt: expose?.prompt,
    expose_resource: expose?.resource,
    expose_openapi: expose?.openapi,
    expose_service: expose?.service,
    expose_cache: expose?.cache,
    expose_search: expose?.search,
  });
  return appApi(`/gateway/wire${query}`, { method: "POST" });
}

export function unwireClient(client: string): Promise<WiredEntry> {
  return appApi(`/gateway/unwire?client=${client}`, { method: "POST" });
}

export function getGatewayStatus(): Promise<GatewayStatus> {
  return appApi("/gateway/status");
}

/** wait_ms 走长轮询（后端上限 25s）。 */
export function getGatewayRequests(
  after: number,
  waitMs?: number,
): Promise<GatewayRequestsPage> {
  return appApi(`/gateway/requests${buildQuery({ after, wait_ms: waitMs })}`);
}

export function getWiredClients(): Promise<WiredEntry[]> {
  return appApi<WiredEntry[]>("/gateway/wired");
}
