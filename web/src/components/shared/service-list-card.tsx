import { BadgeCheckIcon, PlugIcon } from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
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
  const transport = getServiceTransport(service)
  const version = service.mcp?.serverInfo?.version?.trim()
  const endpoint = getServiceEndpointLabel(service)
  const description = String(service.effective_config?.description || "").trim()
  const status = deriveServiceDisplayStatus(service.state)
  const connected = status === "connected"

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group flex h-full min-h-36 cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left shadow-xs transition-colors hover:border-border hover:bg-muted/40",
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
      <div className="flex min-w-0 items-start gap-3">
        <span
          aria-hidden
          className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border/50 bg-muted/60 text-muted-foreground"
        >
          <DefaultServiceIcon />
        </span>
        <span className="min-w-0 flex-1 pt-0.5">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">{service.service_name}</span>
          <span
            className={cn(
              "mt-0.5 flex min-w-0 items-center gap-1 text-xs",
              connected ? "text-emerald-600 dark:text-emerald-500" : "text-muted-foreground",
            )}
          >
            {connected ? <BadgeCheckIcon className="size-3.5 shrink-0" /> : null}
            <span className="truncate">{t(SERVICE_STATUS_LABEL_KEYS[status])}</span>
          </span>
        </span>
      </div>

      <p className="mt-3 line-clamp-2 flex-1 text-xs leading-relaxed text-muted-foreground">
        {description || endpoint || t("noDescription")}
      </p>

      <div className="mt-3 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
        {transport !== "unknown" ? (
          <span className="inline-flex min-w-0 items-center gap-1 text-primary">
            <PlugIcon className="size-3 shrink-0" />
            <span className="truncate lowercase">{transport}</span>
          </span>
        ) : null}
        {toolCount > 0 ? <span className="shrink-0">{t("serviceRowToolCount", { count: toolCount })}</span> : null}
        {version ? <span className="shrink-0 font-mono">v{version}</span> : null}
      </div>
    </div>
  )
}
