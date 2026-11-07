import { useQuery } from "@tanstack/react-query"
import { AlertCircleIcon, BadgeCheckIcon, Loader2Icon } from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { McpIcon } from "@/components/shared/mcp-icon"
import { Skeleton } from "@/components/ui/skeleton"
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
  const scopeLabel =
    service.scope.type === "store" ? "store" : service.scope.agent_id
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
        "group relative z-[1] flex min-h-[9.5rem] cursor-pointer flex-col overflow-hidden rounded-lg border border-border/50 bg-card p-4 text-left",
        "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] transition-[border-color,background-color,box-shadow] duration-[400ms]",
        "hover:z-10 hover:border-sky-500/25 hover:bg-sky-50/90 hover:shadow-none dark:hover:bg-sky-950/40",
        "dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
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
      <div className="flex min-h-0 flex-1 flex-col transition-all duration-[400ms] group-hover:pointer-events-none group-hover:opacity-0">
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
            <span className={cn("mt-1 flex min-w-0 items-center gap-1 text-xs", statusColorClass)}>
              {status === "connected" ? (
                <BadgeCheckIcon className="size-3.5 shrink-0" />
              ) : status === "connecting" ? (
                <Loader2Icon className="size-3.5 shrink-0 animate-spin" />
              ) : status === "error" ? (
                <AlertCircleIcon className="size-3.5 shrink-0" />
              ) : null}
              <span className="truncate">
                {t(SERVICE_STATUS_LABEL_KEYS[status])}
                {connected && transportLabel ? ` · ${transportLabel}` : null}
              </span>
            </span>
          </span>
        </div>

        <div className="mt-3 min-h-10 min-w-0 w-full text-xs leading-5 text-muted-foreground">
          <p className="truncate">{description || t("noDescription")}</p>
          <p className="truncate">
            by <span className="text-[#0077c2]">{scopeLabel}</span>
          </p>
        </div>

        {metaParts.length ? (
          <p className="mt-3 min-w-0 truncate text-xs lowercase text-muted-foreground" title={metaParts.join(" · ")}>
            {metaParts.join(" · ")}
          </p>
        ) : (
          <Skeleton className="mt-3 h-4 w-1/2" aria-hidden />
        )}
      </div>

      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 px-5",
          "opacity-0 transition-all duration-[400ms] [transform:rotate(90deg)_scaleX(-1)]",
          "group-hover:opacity-100 group-hover:[transform:rotate(0deg)_scaleX(1)]",
        )}
      >
        <span className="line-clamp-3 text-center font-mono text-xl font-bold leading-snug tracking-tight text-foreground">
          {service.service_name}
        </span>
        <span className="text-xs text-muted-foreground">
          by <span className="text-[#0077c2]">{scopeLabel}</span>
        </span>
      </div>
    </div>
  )
}
