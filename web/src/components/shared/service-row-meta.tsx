import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { Badge } from "@/components/ui/badge"
import { getServiceTransport } from "@/lib/service-info"
import type { ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"


export function ServiceRowMeta({
  service,
  showScope = false,
  toolCount,
}: {
  service: ServiceInstance
  showScope?: boolean
  toolCount?: number
}) {
  const { t } = useI18n()
  const scope =
    service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`
  const transport = getServiceTransport(service)
  const transportLabel = transport !== "unknown" ? transport : "-"

  return (
    <div className="mt-1 flex min-w-0 flex-wrap items-center gap-2 font-mono text-xs text-muted-foreground">
      {showScope ? (
        <Badge variant="outline" className="max-w-full font-mono">
          <span className="truncate">{scope}</span>
        </Badge>
      ) : null}
      <Badge variant="outline" className="shrink-0 font-mono">
        {transportLabel}
      </Badge>
      <ServiceStatusBadge state={service.state} />
      {toolCount !== undefined ? (
        <Badge variant="outline" className="shrink-0 font-mono">
          {t("serviceRowToolCount", { count: toolCount })}
        </Badge>
      ) : null}
    </div>
  )
}
