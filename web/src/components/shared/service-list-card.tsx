import { useQuery } from "@tanstack/react-query"
import { AlertCircleIcon, BadgeCheckIcon, Loader2Icon } from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { McpIcon } from "@/components/shared/mcp-icon"
import { deriveServiceDisplayStatus, isServiceConnected } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import { listInstancePrompts, listInstanceResources, type ServiceInstance } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { getServiceEndpointLabel, getServiceTransport } from "@/lib/service-info"
import { cn } from "@/lib/utils"

export function ServiceListCard({
  className,
  onSelect,
  service,
  toolCount,
}: {
  className?: string
  onSelect: () => void
  service: ServiceInstance
  toolCount: number
}) {
  const { t } = useI18n()
  const endpoint = getServiceEndpointLabel(service)
  const description = String(service.effective_config?.description || "").trim() || endpoint
  const status = deriveServiceDisplayStatus(service.state)
  const connected = status === "connected"
  const transport = getServiceTransport(service)
  const transportLabel =
    transport === "stdio" ? "stdio" : transport.includes("http") ? "http" : null

  const canListResources = isServiceConnected(service.state) && service.mcp?.capabilities?.resources !== false
  const canListPrompts = isServiceConnected(service.state) && service.mcp?.capabilities?.prompts !== false

  const resourcesQuery = useQuery({
    enabled: canListResources,
    queryKey: queryKeys.instanceResources(service),
    queryFn: () => listInstanceResources(service).catch(() => []),
    staleTime: 60_000,
  })
  const promptsQuery = useQuery({
    enabled: canListPrompts,
    queryKey: queryKeys.instancePrompts(service),
    queryFn: () => listInstancePrompts(service).catch(() => []),
    staleTime: 60_000,
  })

  const resourceCount = resourcesQuery.data?.length ?? 0
  const promptCount = promptsQuery.data?.length ?? 0
  const metaParts = [
    toolCount > 0 ? t("serviceRowToolCount", { count: toolCount }) : null,
    resourceCount > 0 ? t("serviceRowResourceCount", { count: resourceCount }) : null,
    promptCount > 0 ? t("serviceRowPromptCount", { count: promptCount }) : null,
  ].filter((part): part is string => Boolean(part))

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group flex cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left transition-colors hover:border-border hover:bg-muted/40",
        className,
      )}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <div className="flex min-w-0 items-start gap-2">
        <span
          aria-hidden
          className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground"
        >
          <McpIcon
            icons={service.mcp?.serverInfo?.icons}
            fallback={<DefaultServiceIcon />}
            className="size-4"
          />
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">
            {service.service_name}
          </span>
          <span className="mt-1 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
            {status === "connected" ? (
              <BadgeCheckIcon className="size-3.5 shrink-0" />
            ) : status === "connecting" ? (
              <Loader2Icon className="size-3.5 shrink-0 animate-spin" />
            ) : status === "error" ? (
              <AlertCircleIcon className="size-3.5 shrink-0 text-destructive" />
            ) : null}
            <span className="truncate">
              {t(SERVICE_STATUS_LABEL_KEYS[status])}
              {connected && transportLabel ? ` · ${transportLabel}` : null}
            </span>
          </span>
        </span>
      </div>

      {description ? (
        <p className="mt-3 min-w-0 w-full truncate text-sm text-muted-foreground">
          {description}
        </p>
      ) : null}

      {metaParts.length ? (
        <p className="mt-3 min-w-0 truncate text-xs lowercase text-muted-foreground" title={metaParts.join(" · ")}>
          {metaParts.join(" · ")}
        </p>
      ) : null}
    </div>
  )
}
