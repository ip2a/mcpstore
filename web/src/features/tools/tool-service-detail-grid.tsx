import { MetaLine } from "@/components/shared/meta-line"
import { useI18n } from "@/lib/i18n-context"
import { formatDateTime } from "@/lib/format"
import { findToolStatus, getToolOutputSchema, getToolSchema } from "@/lib/tool-info"
import type { ServiceEntry, ServiceStatusReport, ToolInfo } from "@/lib/api"

export function ToolServiceDetailGrid({
  tool,
  service,
  statusReport,
  sourceLabel,
}: {
  tool: ToolInfo
  service: ServiceEntry
  statusReport?: ServiceStatusReport | null
  sourceLabel: string
}) {
  const { t } = useI18n()
  const schema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
  const outputSchema = getToolOutputSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
  const paramCount = Object.keys(schema.properties || {}).length
  const outputParamCount = Object.keys(outputSchema.properties || {}).length
  const toolStatus = findToolStatus(tool.name, statusReport)
  const config = service.config as Record<string, unknown> | undefined
  const configArgs = Array.isArray(config?.args) ? config.args.map(String).join(" ") : null

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-2 text-sm sm:grid-cols-2">
        <MetaLine label={t("params")} value={`${paramCount} · ${t("schemaFields")}`} valueClassName="font-mono" />
        <MetaLine label={t("required")} value={`${schema.required?.length || 0} · ${t("mandatory")}`} valueClassName="font-mono" />
        <MetaLine label={t("output")} value={`${outputParamCount} · ${t("schemaFields")}`} valueClassName="font-mono" />
        <MetaLine label={t("source")} value={`${sourceLabel} · ${t("service")}`} valueClassName="font-mono" />
        {statusReport ? (
          <>
            <MetaLine label={t("health")} value={t("healthServiceRuntime", { status: statusReport.health_status || "-" })} valueClassName="font-mono capitalize" />
            <MetaLine label={t("toolStatusLabel")} value={t("toolStatusAvailability", { status: toolStatus?.status || t("unknown") })} valueClassName="font-mono capitalize" />
            <MetaLine label={t("globalName")} value={`${toolStatus?.tool_global_name || tool.name} · ${t("registeredId")}`} valueClassName="font-mono" />
          </>
        ) : null}
        <MetaLine label={t("name")} value={service.name} valueClassName="font-mono" />
        <MetaLine label={t("original")} value={service.original_name || service.name} valueClassName="font-mono" />
        <MetaLine label={t("status")} value={service.status || "-"} valueClassName="font-mono" />
        <MetaLine label={t("transport")} value={String(service.transport || config?.transport || "-")} valueClassName="font-mono" />
        <MetaLine label={t("command")} value={String(service.command || config?.command || "-")} valueClassName="font-mono" />
        {configArgs ? <MetaLine label={t("args")} value={configArgs} valueClassName="font-mono" /> : null}
        <MetaLine label={t("endpoint")} value={String(service.url || service.command || "-")} valueClassName="font-mono" />
        <MetaLine label={t("agent")} value={String(service.agent_id || t("store"))} valueClassName="font-mono" />
        <MetaLine label={t("added")} value={formatDateTime(service.added_time)} valueClassName="font-mono" />
        <MetaLine label={t("toolCount")} value={String(service.tool_count ?? service.tools?.length ?? "-")} valueClassName="font-mono" />
        <MetaLine label={t("clientId")} value={String(service.client_id || "-")} valueClassName="font-mono" />
      </div>

      {statusReport ? (
        <div className="border-t pt-4">
          <p className="mb-3 text-sm font-medium">{t("runtimeStatus")}</p>
          <div className="grid gap-2 text-sm sm:grid-cols-2">
            <MetaLine label={t("health")} value={statusReport.health_status || "-"} valueClassName="font-mono capitalize" />
            <MetaLine label={t("lastCheck")} value={formatDateTime(statusReport.last_health_check)} valueClassName="font-mono" />
            <MetaLine
              label={t("connections")}
              value={`${statusReport.connection_attempts ?? 0} / ${statusReport.max_connection_attempts ?? "-"}`}
              valueClassName="font-mono"
            />
            <MetaLine label={t("errorRate")} value={statusReport.window_error_rate ?? "-"} valueClassName="font-mono" />
            <MetaLine label={t("latencyP95")} value={statusReport.latency_p95 ?? "-"} valueClassName="font-mono" />
            <MetaLine label={t("latencyP99")} value={statusReport.latency_p99 ?? "-"} valueClassName="font-mono" />
            {toolStatus ? (
              <MetaLine label={t("toolGlobal")} value={toolStatus.tool_global_name || "-"} valueClassName="font-mono" />
            ) : null}
            {statusReport.current_error ? (
              <MetaLine label={t("currentError")} value={statusReport.current_error} destructive valueClassName="font-mono" />
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}
