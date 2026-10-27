import { BadgeCheckIcon, DownloadIcon, PuzzleIcon } from "lucide-react"

import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export type PluginItem = {
  id: string
  name: string
  title: string
  description: string
  version: string
  author: string
  installed: boolean
}

export function PluginListCard({
  className,
  onSelect,
  plugin,
}: {
  className?: string
  onSelect?: () => void
  plugin: PluginItem
}) {
  const { t } = useI18n()
  const displayName = plugin.title.trim() || plugin.name

  return (
    <div
      role={onSelect ? "button" : undefined}
      tabIndex={onSelect ? 0 : undefined}
      className={cn(
        "group flex h-full min-h-36 flex-col rounded-lg border border-border/50 bg-card p-4 text-left shadow-xs transition-colors",
        onSelect && "cursor-pointer hover:border-border hover:bg-muted/40",
        className,
      )}
      onClick={onSelect}
      onKeyDown={
        onSelect
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                onSelect()
              }
            }
          : undefined
      }
    >
      <div className="flex min-w-0 items-start gap-3">
        <span
          aria-hidden
          className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border/50 bg-muted/60 text-muted-foreground"
        >
          <PuzzleIcon className="size-5" />
        </span>
        <span className="min-w-0 flex-1 pt-0.5">
          <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">{displayName}</span>
          <span
            className={cn(
              "mt-0.5 flex min-w-0 items-center gap-1 text-xs",
              plugin.installed
                ? "text-emerald-600 dark:text-emerald-500"
                : "text-muted-foreground",
            )}
          >
            {plugin.installed ? (
              <BadgeCheckIcon className="size-3.5 shrink-0" />
            ) : (
              <DownloadIcon className="size-3.5 shrink-0" />
            )}
            <span className="truncate">{plugin.author}</span>
          </span>
        </span>
      </div>

      <p className="mt-3 line-clamp-2 flex-1 text-xs leading-relaxed text-muted-foreground">
        {plugin.description.trim() || t("noDescription")}
      </p>

      <div className="mt-3 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
        <span className="shrink-0 font-mono">v{plugin.version}</span>
        <span className="shrink-0">
          {plugin.installed ? t("pluginInstalled") : t("pluginsTabNotInstalled")}
        </span>
      </div>
    </div>
  )
}
