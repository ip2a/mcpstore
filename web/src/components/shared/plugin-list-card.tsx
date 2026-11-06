import { PuzzleIcon } from "lucide-react"

import { clientIconUrls } from "@/assets/icons/clients"
import { Button } from "@/components/ui/button"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export type PluginKind = "service" | "agent"

export type PluginItem = {
  id: string
  name: string
  title: string
  description: string
  version: string
  author: string
  installed: boolean
  kind: PluginKind
  updateAvailable?: boolean
}

export function PluginListCard({
  busy = false,
  className,
  onInstall,
  onUpdate,
  plugin,
}: {
  busy?: boolean
  className?: string
  onInstall?: (id: string) => void
  onUpdate?: (id: string) => void
  plugin: PluginItem
}) {
  const { t } = useI18n()
  const displayName = plugin.title.trim() || plugin.name
  const iconUrl = clientIconUrls[plugin.id]
  const updateAvailable = Boolean(plugin.updateAvailable)
  const description = plugin.description.trim()
  const statusText = plugin.installed
    ? updateAvailable
      ? t("clientPluginUpdateAvailable", { version: plugin.version })
      : t("clientPluginInstalled")
    : t("clientPluginInstall")
  const canInstall = Boolean(onInstall) && !plugin.installed
  const canUpdate = Boolean(onUpdate) && plugin.installed && updateAvailable

  return (
    <div
      className={cn(
        "group flex flex-col rounded-lg border border-border/50 bg-card p-4 text-left",
        "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
        "transition-colors hover:border-border hover:bg-muted/40",
        className,
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground">
          {iconUrl ? (
            <img src={iconUrl} alt="" className="size-5 object-contain" aria-hidden />
          ) : (
            <PuzzleIcon className="size-4" aria-hidden />
          )}
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 text-sm font-semibold leading-snug">{displayName}</span>
          <span className="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                plugin.installed
                  ? updateAvailable
                    ? "bg-amber-500"
                    : "bg-emerald-500"
                  : "bg-muted-foreground/40",
              )}
            />
            <span className="truncate">{statusText}</span>
          </span>
        </span>
        {plugin.installed ? (
          <Button
            size="sm"
            variant="outline"
            className="shrink-0"
            disabled={busy || !canUpdate}
            onClick={() => onUpdate?.(plugin.id)}
          >
            {updateAvailable ? t("clientPluginUpdate") : t("clientPluginInstalled")}
          </Button>
        ) : (
          <Button
            size="sm"
            className="shrink-0"
            disabled={busy || !canInstall}
            onClick={() => onInstall?.(plugin.id)}
          >
            {t("clientPluginInstall")}
          </Button>
        )}
      </div>

      <p className="mt-3 min-h-10 min-w-0 w-full line-clamp-2 text-xs leading-5 text-muted-foreground">
        {description || t("noDescription")}
      </p>

      {plugin.version ? (
        <p className="mt-3 font-mono text-xs tabular-nums text-muted-foreground">
          {plugin.version}
        </p>
      ) : null}
    </div>
  )
}
