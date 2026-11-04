import { BadgeCheckIcon } from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { McpIcon } from "@/components/shared/mcp-icon"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { getServiceEndpointLabel } from "@/lib/service-info"
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
            {connected ? <BadgeCheckIcon className="size-3.5 shrink-0" /> : null}
            <span className="truncate">{t(SERVICE_STATUS_LABEL_KEYS[status])}</span>
          </span>
        </span>
      </div>

      {description ? (
        <p className="mt-3 min-w-0 w-full truncate text-sm text-muted-foreground">
          {description}
        </p>
      ) : null}

      {toolCount > 0 ? (
        <p className="mt-3 text-xs text-muted-foreground">
          {t("serviceRowToolCount", { count: toolCount })}
        </p>
      ) : null}
    </div>
  )
}
