import { BadgeCheckIcon, ScrollTextIcon } from "lucide-react"

import { McpIcon } from "@/components/shared/mcp-icon"
import { promptArgCount } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { PromptInfo, ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

export function PromptListCard({
  className,
  instance,
  onSelect,
  prompt,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  prompt: PromptInfo
}) {
  const { t } = useI18n()
  const displayName = prompt.title?.trim() || prompt.name
  const description = prompt.description?.trim()
  const argCount = promptArgCount(prompt)

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
          <McpIcon icons={prompt.icons} fallback={<ScrollTextIcon className="size-4" />} className="size-4" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">{displayName}</span>
          <span className="mt-1 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
            <BadgeCheckIcon className="size-3.5 shrink-0" />
            <span className="truncate">{instance.service_name}</span>
          </span>
        </span>
      </div>

      {description ? (
        <p className="mt-3 min-w-0 w-full truncate text-sm text-muted-foreground">
          {description}
        </p>
      ) : null}

      <p className="mt-3 text-xs text-muted-foreground">
        {t("paramCount", { count: argCount })}
      </p>
    </div>
  )
}
