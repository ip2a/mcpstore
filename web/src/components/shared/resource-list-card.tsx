import { BadgeCheckIcon, FileStackIcon, PlugIcon } from "lucide-react"

import { McpIcon } from "@/components/shared/mcp-icon"
import { resourceMimeType } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ResourceInfo, ServiceInstance } from "@/lib/api"
import { getServiceTransport } from "@/lib/service-info"
import { cn } from "@/lib/utils"

export function ResourceListCard({
  className,
  instance,
  onSelect,
  resource,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  resource: ResourceInfo
}) {
  const { t } = useI18n()
  const transport = getServiceTransport(instance)
  const version = instance.mcp?.serverInfo?.version?.trim()
  const displayName = resource.title?.trim() || resource.name
  const mime = resourceMimeType(resource)

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
          <McpIcon icons={resource.icons} fallback={<FileStackIcon className="size-5" />} className="size-5" />
        </span>
        <span className="min-w-0 flex-1 pt-0.5">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">{displayName}</span>
          <span className="mt-0.5 flex min-w-0 items-center gap-1 text-xs text-emerald-600 dark:text-emerald-500">
            <BadgeCheckIcon className="size-3.5 shrink-0" />
            <span className="truncate">{instance.service_name}</span>
          </span>
        </span>
      </div>

      <p className="mt-3 line-clamp-2 flex-1 text-xs leading-relaxed text-muted-foreground">
        {resource.description?.trim() || resource.uri || t("noDescription")}
      </p>

      <div className="mt-3 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
        {transport !== "unknown" ? (
          <span className="inline-flex min-w-0 items-center gap-1 text-primary">
            <PlugIcon className="size-3 shrink-0" />
            <span className="truncate lowercase">{transport}</span>
          </span>
        ) : null}
        {mime ? <span className="shrink-0 font-mono">{mime}</span> : null}
        {version ? <span className="shrink-0 font-mono">v{version}</span> : null}
      </div>
    </div>
  )
}
