import { isServiceConnected } from "@/features/services/service-display-status"
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
  const showToolCount = toolCount !== undefined && isServiceConnected(service.state)

  const parts = [
    showScope ? scope : null,
    transportLabel,
    showToolCount ? t("serviceRowToolCount", { count: toolCount }) : null,
  ].filter((part): part is string => Boolean(part))

  return (
    <div className="mt-1 min-w-0 truncate text-sm text-muted-foreground" title={parts.join(" · ")}>
      {parts.join("  ")}
    </div>
  )
}
