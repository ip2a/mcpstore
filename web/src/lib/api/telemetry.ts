// 本模块全部为 app 自有接口（daemon 请求遥测持久层：历史/详情/统计），
// 固定走 appApi，不随 core 后端切换。
import { appApi, buildQuery } from "./client";

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
  return appApi(`/gateway/stats${buildQuery({ hours })}`);
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
  return appApi(`/gateway/requests/history${buildQuery(params)}`);
}

export function getRequestDetail(id: number): Promise<RequestRecord> {
  return appApi(`/gateway/requests/detail/${id}`);
}
