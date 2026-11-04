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

/** Server-paginated request history list. */
export type HistoryPage = {
  requests: RequestRecord[];
  total: number;
  limit: number;
  offset: number;
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

/** Daily tool call totals (UTC day buckets); heatmap data source. */
export function getDailyUsageStats(days = 35): Promise<UsageTotals[]> {
  return api(`/gateway/stats/daily${buildQuery({ days })}`);
}

/** Daily primitive call counts from frame capture (gateway face); home heatmap data source. */
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

/** Frame-based activity stats over a flexible time range; bucketMs overrides the range-based default. */
export function getFrameActivityStats(
  hours = 24,
  bucketMs?: number,
): Promise<HeatmapDay[]> {
  return api(
    `/gateway/stats/frame-activity${buildQuery({ hours, bucket_ms: bucketMs })}`,
  );
}

export type ServiceActivityPoint = {
  bucket_unix_ms: number;
  /** Scope-qualified display label resolved by the API (`store/demo1`, `<agent>/demo1`). */
  service: string;
  instance_id: string | null;
  frame_count: number;
};

/** Frame activity grouped by service instance over a time range. */
export function getFrameActivityByService(
  hours = 24,
  bucketMs?: number,
): Promise<ServiceActivityPoint[]> {
  return api(
    `/gateway/stats/frame-activity-by-service${buildQuery({ hours, bucket_ms: bucketMs })}`,
  );
}

export type MethodActivityPoint = {
  bucket_unix_ms: number;
  method: string;
  frame_count: number;
};

/** Frame activity grouped by wire method — every series is a real method name. */
export function getFrameActivityByMethod(
  hours = 24,
  bucketMs?: number,
): Promise<MethodActivityPoint[]> {
  return api(
    `/gateway/stats/frame-activity-by-method${buildQuery({ hours, bucket_ms: bucketMs })}`,
  );
}

export function getRequestHistory(
  params: {
    limit?: number;
    offset?: number;
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

/** Frame session summary for one connection (handshake → disconnect, mount-level aggregation). */
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

/** Server-paginated frame session list. */
export type FrameSessionsPage = {
  sessions: FrameSessionSummary[];
  total: number;
  limit: number;
  offset: number;
};

/** A raw JSON-RPC frame. up = client→server direction, down = server→client. */
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
