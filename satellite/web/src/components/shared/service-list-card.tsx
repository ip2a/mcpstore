import { AlertCircleIcon, BadgeCheckIcon, Loader2Icon } from "lucide-react"

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
  // Failure detail stays off the face (keeps card height stable); show on hover flip.
  const failureMessage =
    status !== "connected" ? service.state?.failure?.message?.trim() || null : null
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
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group relative z-[1] flex h-[9.5rem] cursor-pointer flex-col overflow-hidden rounded-lg border border-border/50 bg-card p-4 text-left",
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
            <McpIcon icons={icons} fallback={<DefaultServiceIcon />} className="size-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">
              {projectedName}
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
          <p
            className="mt-3 min-w-0 truncate text-xs lowercase text-muted-foreground"
            title={metaParts.join(" · ")}
          >
            {metaParts.join(" · ")}
          </p>
        ) : (
          <p className="mt-3 min-h-4 text-xs text-muted-foreground/60">&nbsp;</p>
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
          {projectedName}
        </span>
        <span className="text-xs text-muted-foreground">
          by <span className="text-[#0077c2]">{scopeLabel}</span>
        </span>
        {failureMessage ? (
          <span
            className={cn(
              "mt-1 line-clamp-4 max-w-full text-center text-xs leading-5",
              status === "error" ? "text-destructive" : "text-amber-600 dark:text-amber-400",
            )}
            title={failureMessage}
          >
            {failureMessage}
          </span>
        ) : null}
      </div>
    </div>
  )
}
