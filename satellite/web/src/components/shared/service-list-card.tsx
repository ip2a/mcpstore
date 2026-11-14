import { AlertCircleIcon, BadgeCheckIcon, Loader2Icon } from "lucide-react"

import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { McpIcon } from "@/components/shared/mcp-icon"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { getServiceEndpointLabel, getServiceTransport } from "@/lib/service-info"
import { cn } from "@/lib/utils"

function serviceToolCount(service: ServiceInstance) {
  return service.tool_count ?? service.tools?.length ?? 0
}

export function ServiceListCard({
  className,
  onSelect,
  service,
  toolCount,
}: {
  className?: string
  onSelect: () => void
  service: ServiceInstance
  toolCount?: number
}) {
  const { t } = useI18n()
  const endpoint = getServiceEndpointLabel(service)
  const projectedName = service.display_name?.trim() || service.service_name
  const description =
    service.description?.trim() ||
    String(service.effective_config?.description || "").trim() ||
    endpoint
  const scopeLabel = service.scope.type === "store" ? "store" : service.scope.agent_id
  const status = deriveServiceDisplayStatus(service.state)
  const connected = status === "connected"
  const statusColorClass =
    status === "connected"
      ? "text-emerald-500"
      : status === "connecting"
        ? "text-amber-500"
        : status === "error"
          ? "text-destructive"
          : "text-muted-foreground"
  const transport = getServiceTransport(service)
  const transportLabel =
    transport === "stdio" ? "stdio" : transport.includes("http") ? "http" : null

  const resolvedToolCount = toolCount ?? serviceToolCount(service)
  const metaParts = [
    resolvedToolCount > 0 ? t("serviceRowToolCount", { count: resolvedToolCount }) : null,
  ].filter((part): part is string => Boolean(part))

  const icons = service.icons ?? service.mcp?.serverInfo?.icons

  return (
    <BasicCard
      className={className}
      onClick={onSelect}
      icon={
        <McpIcon
          icons={icons}
          fallback={<DefaultServiceIcon className={basicCardIconClassName} />}
          className={basicCardIconClassName}
        />
      }
      title={projectedName}
      subtitle={
        <>
          {status === "connected" ? (
            <BadgeCheckIcon className={cn("size-3.5 shrink-0", statusColorClass)} />
          ) : status === "connecting" ? (
            <Loader2Icon className={cn("size-3.5 shrink-0 animate-spin", statusColorClass)} />
          ) : status === "error" ? (
            <AlertCircleIcon className={cn("size-3.5 shrink-0", statusColorClass)} />
          ) : null}
          <span className={cn("truncate", statusColorClass)}>
            {t(SERVICE_STATUS_LABEL_KEYS[status])}
            {connected && transportLabel ? ` · ${transportLabel}` : null}
          </span>
        </>
      }
      body={
        <>
          <p className="truncate">{description || t("noDescription")}</p>
          {metaParts.length ? (
            <p className="truncate lowercase" title={metaParts.join(" · ")}>
              {metaParts.join(" · ")}
            </p>
          ) : (
            <p className="truncate text-muted-foreground/60">&nbsp;</p>
          )}
        </>
      }
      footer={
        <p className="truncate">
          by <span className="text-[#0077c2]">{scopeLabel}</span>
        </p>
      }
    />
  )
}
