// Resource telemetry follows the selected daemon through the App face.
import { api, buildQuery } from "./client";

export type RequestRecord = {
  id: number;
  request_id: string;
  started_at_unix_ms: number;
  duration_ms: number;
  kind: string;
  caller: string | null;
  scope: string | null;
  session_key: string | null;
  instance_id: string | null;
  service_name: string | null;
  tool: string | null;
  success: boolean;
  error_code: string | null;
  error_summary: string | null;
  args_json: string | null;
  result_json: string | null;
  args_truncated: boolean;
  result_truncated: boolean;
};

export type HistoryPage = {
  requests: RequestRecord[];
  has_more: boolean;
};

export type UsageTotals = {
  bucket_unix_ms: number;
  calls: number;
  errors: number;
  avg_ms: number;
  max_ms: number;
};

export type UsageSeriesPoint = UsageTotals & { label: string };

export type UsageStats = {
  from_unix_ms: number;
  to_unix_ms: number;
  by_hour: UsageTotals[];
  by_caller: UsageSeriesPoint[];
  by_service: UsageSeriesPoint[];
  by_tool: UsageSeriesPoint[];
  by_instance: UsageSeriesPoint[];
};

export function getUsageStats(hours = 24): Promise<UsageStats> {
  return api(`/gateway/stats${buildQuery({ hours })}`);
}

/** 每日工具调用总量（UTC 天桶），热力图数据源。 */
export function getDailyUsageStats(days = 35): Promise<UsageTotals[]> {
  return api(`/gateway/stats/daily${buildQuery({ days })}`);
}

/** 帧捕获维度的每日原语调用情况（gateway 面），首页热力图数据源。 */
export type HeatmapDay = {
  bucket_unix_ms: number;
  tool_calls: number;
  resource_calls: number;
  prompt_calls: number;
  other_calls: number;
  errors: number;
};

export function getHeatmapStats(days = 371): Promise<HeatmapDay[]> {
  return api(`/gateway/stats/heatmap${buildQuery({ days })}`);
}

/** Frame-based activity stats over a flexible time range. */
export function getFrameActivityStats(hours = 24): Promise<HeatmapDay[]> {
  return api(`/gateway/stats/frame-activity${buildQuery({ hours })}`);
}

export function getRequestHistory(
  params: {
    limit?: number;
    caller?: string;
    service?: string;
    tool?: string;
    success?: boolean;
    before_id?: number;
  } = {},
): Promise<HistoryPage> {
  return api(`/gateway/requests/history${buildQuery(params)}`);
}

export function getRequestDetail(id: number): Promise<RequestRecord> {
  return api(`/gateway/requests/detail/${id}`);
}

/** 一次连接（握手→断开/挂载级聚合）的帧会话摘要。 */
export type FrameSessionSummary = {
  session_id: string;
  face: "gateway" | "upstream";
  caller: string | null;
  peer: string | null;
  instance_id: string | null;
  first_ts_unix_ms: number;
  last_ts_unix_ms: number;
  frame_count: number;
};

/** 服务端分页的帧会话列表。 */
export type FrameSessionsPage = {
  sessions: FrameSessionSummary[];
  total: number;
  limit: number;
  offset: number;
};

/** 一条原始 JSON-RPC 帧。up=client→server 方向，down=server→client。 */
export type FrameRecord = {
  id: number;
  session_id: string;
  ts_unix_ms: number;
  face: "gateway" | "upstream";
  direction: "up" | "down";
  method: string | null;
  jsonrpc_id: string | null;
  kind: "request" | "response" | "notification" | "error";
  payload: string;
  size_bytes: number;
  truncated: boolean;
  caller: string | null;
  instance_id: string | null;
  peer: string | null;
  transport: string | null;
};

export function getFrameSessions(
  params: {
    face?: string;
    hours?: number;
    limit?: number;
    offset?: number;
  } = {},
): Promise<FrameSessionsPage> {
  return api(`/gateway/frames/sessions${buildQuery(params)}`);
}

export function getFrameHistory(
  sessionId: string,
  limit = 200,
): Promise<FrameRecord[]> {
  return api(
    `/gateway/frames/history${buildQuery({ session_id: sessionId, limit })}`,
  );
}

/** Recent frames across sessions, oldest→newest in the returned window. */
export function getRecentFrames(
  params: { face?: string; limit?: number } = {},
): Promise<FrameRecord[]> {
  return api(`/gateway/frames/recent${buildQuery(params)}`);
}

export function getFrameDetail(id: number): Promise<FrameRecord> {
  return api(`/gateway/frames/detail/${id}`);
}
