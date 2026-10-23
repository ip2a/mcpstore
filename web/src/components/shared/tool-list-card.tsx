import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolInfo } from "@/lib/api"
import { cn } from "@/lib/utils"

export function ToolListCard({
  className,
  instance,
  onSelect,
  tool,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  tool: ToolInfo
}) {
  const { t } = useI18n()

  return (
    <button
      type="button"
      className={cn(
        "group flex h-full min-h-32 flex-col rounded-lg border border-border/50 bg-card p-4 text-left shadow-xs transition-colors hover:border-border hover:bg-muted/40",
        className,
      )}
      onClick={onSelect}
    >
      <span className="min-w-0 flex-1">
        <span className="line-clamp-2 font-mono text-sm font-semibold leading-snug">{tool.name}</span>
        <span className="mt-1.5 line-clamp-2 text-xs leading-relaxed text-muted-foreground">
          {tool.description?.trim() || t("noDescription")}
        </span>
      </span>
      <span className="mt-3 truncate font-mono text-[11px] text-muted-foreground">{instance.service_name}</span>
    </button>
  )
}
