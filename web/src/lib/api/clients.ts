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

export type WiredEntry = {
  client: string;
  path: string;
  name: string;
  url: string;
  applied_at_unix_ms: number;
};

export function listClients(): Promise<ClientInventory[]> {
  return appApi<{ clients: ClientInventory[] }>("/clients").then(
    (payload) => payload.clients,
  );
}

/** path/url 省略时由后端发现配置文件并生成 caller URL。 */
export function wireClient(client: string): Promise<WiredEntry> {
  return appApi(`/gateway/wire?client=${client}`, { method: "POST" });
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
