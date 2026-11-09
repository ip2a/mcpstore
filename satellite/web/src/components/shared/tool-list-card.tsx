import { BadgeCheckIcon } from "lucide-react"

import { DefaultToolIcon } from "@/components/shared/default-tool-icon"
import { McpIcon } from "@/components/shared/mcp-icon"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolInfo } from "@/lib/api"
import { getToolSchema } from "@/lib/tool-info"
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
  const displayName = tool.title?.trim() || tool.name
  const schema = getToolSchema(tool) as { properties?: Record<string, unknown> }
  const paramCount = Object.keys(schema.properties || {}).length
  const description = tool.description?.trim()
  const scopeLabel =
    instance.scope.type === "store" ? "store" : instance.scope.agent_id

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group flex cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left",
        "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
        "transition-colors hover:border-border hover:bg-muted/40",
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
          <McpIcon icons={tool.icons} fallback={<DefaultToolIcon />} className="size-4" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">
            {displayName}
          </span>
          <span className="mt-1 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
            <BadgeCheckIcon className="size-3.5 shrink-0" />
            <span className="truncate">{instance.service_name}</span>
          </span>
        </span>
      </div>

      <p className="mt-3 min-h-10 min-w-0 w-full line-clamp-2 text-xs leading-5 text-muted-foreground">
        {description || t("noDescription")}
      </p>

      <p className="mt-3 min-w-0 truncate text-xs text-muted-foreground">
        {t("paramCount", { count: paramCount })}
        {" · by "}
        <span className="text-[#0077c2]">{scopeLabel}</span>
      </p>
    </div>
  )
}
