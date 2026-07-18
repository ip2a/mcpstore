import { MetaLine } from "@/components/shared/meta-line"
import { useI18n } from "@/lib/i18n-context"
import { formatDateTime } from "@/lib/format"
import { findToolStatus, getToolOutputSchema, getToolSchema } from "@/lib/tool-info"
import type { ServiceInstance, ServiceState, ToolInfo } from "@/lib/api"

export function ToolServiceDetailGrid({
  tool,
  service,
  statusReport,
  sourceLabel,
}: {
  tool: ToolInfo
  service: ServiceInstance
  statusReport?: ServiceState | null
  sourceLabel: string
}) {
  const { t } = useI18n()
  const schema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
  const outputSchema = getToolOutputSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
  const paramCount = Object.keys(schema.properties || {}).length
  const outputParamCount = Object.keys(outputSchema.properties || {}).length
  const toolStatus = findToolStatus(tool.name, statusReport)
  const config = service.effective_config
  const configArgs = Array.isArray(config.args) ? config.args.map(String).join(" ") : null
  const scopeLabel = service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`

  return (
    <div className="@container flex flex-col gap-4">
      <div className="grid gap-2 text-sm @min-[32rem]:grid-cols-2">
        <MetaLine label={t("params")} value={`${paramCount} · ${t("schemaFields")}`} valueClassName="font-mono" />
        <MetaLine label={t("required")} value={`${schema.required?.length || 0} · ${t("mandatory")}`} valueClassName="font-mono" />
        <MetaLine label={t("output")} value={`${outputParamCount} · ${t("schemaFields")}`} valueClassName="font-mono" />
        <MetaLine label={t("source")} value={`${sourceLabel} · ${t("service")}`} valueClassName="font-mono" />
        {statusReport ? (
          <>
            <MetaLine label={t("health")} value={t("healthServiceRuntime", { status: statusReport.health || "-" })} valueClassName="font-mono capitalize" />
            <MetaLine label={t("toolStatusLabel")} value={t("toolStatusAvailability", { status: toolStatus?.availability || t("unknown") })} valueClassName="font-mono capitalize" />
            <MetaLine label={t("tool")} value={toolStatus?.name || tool.name} valueClassName="font-mono" />
          </>
        ) : null}
        <MetaLine label="Instance ID" value={service.instance_id} valueClassName="font-mono" />
        <MetaLine label={t("service")} value={service.service_name} valueClassName="font-mono" />
        <MetaLine label={t("scope")} value={scopeLabel} valueClassName="font-mono" />
        <MetaLine label={t("status")} value={service.state.readiness.status} valueClassName="font-mono" />
        <MetaLine label={t("transport")} value={service.transport} valueClassName="font-mono" />
        <MetaLine label={t("command")} value={service.command || "-"} valueClassName="font-mono" />
        {configArgs ? <MetaLine label={t("args")} value={configArgs} valueClassName="font-mono" /> : null}
        <MetaLine label={t("endpoint")} value={String(service.url || service.command || "-")} valueClassName="font-mono" />
        <MetaLine label={t("added")} value={formatDateTime(service.added_time)} valueClassName="font-mono" />
        <MetaLine label={t("toolCount")} value={String(service.tools.length)} valueClassName="font-mono" />
      </div>

      {statusReport ? (
        <div className="border-t pt-4">
          <p className="mb-3 text-sm font-medium">{t("runtimeStatus")}</p>
          <div className="grid gap-2 text-sm @min-[32rem]:grid-cols-2">
            <MetaLine label={t("health")} value={statusReport.health} valueClassName="font-mono capitalize" />
            <MetaLine label={t("lastCheck")} value={formatDateTime(statusReport.last_observed_at)} valueClassName="font-mono" />
            <MetaLine label={t("status")} value={statusReport.recovery.status} valueClassName="font-mono capitalize" />
            <MetaLine label={t("errorRate")} value={statusReport.health_metrics.error_rate ?? "-"} valueClassName="font-mono" />
            <MetaLine label={t("latencyP95")} value={statusReport.health_metrics.latency_p95_ms ?? "-"} valueClassName="font-mono" />
            <MetaLine label={t("latencyP99")} value={statusReport.health_metrics.latency_p99_ms ?? "-"} valueClassName="font-mono" />
            {toolStatus ? <MetaLine label={t("tool")} value={toolStatus.name} valueClassName="font-mono" /> : null}
            {statusReport.failure ? (
              <MetaLine label={t("currentError")} value={statusReport.failure.message} destructive valueClassName="font-mono" />
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}
