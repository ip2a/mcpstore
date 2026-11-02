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
        "group flex h-full min-h-36 flex-col rounded-lg border border-border/50 bg-card p-4 text-left shadow-xs transition-colors hover:border-border hover:bg-muted/40",
        className,
      )}
    >
      <div className="flex min-w-0 items-center gap-3">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-border/50 bg-muted/60 text-muted-foreground">
          {iconUrl ? (
            <img src={iconUrl} alt="" className="size-7 object-contain" aria-hidden />
          ) : (
            <PuzzleIcon className="size-5" aria-hidden />
          )}
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 text-sm font-semibold leading-snug">{displayName}</span>
          <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
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

      <p className="mt-3 line-clamp-2 flex-1 text-xs leading-relaxed text-muted-foreground">
        {plugin.description.trim() || t("noDescription")}
      </p>

      <div className="mt-3 font-mono text-[11px] tabular-nums text-muted-foreground">
        {plugin.version}
      </div>
    </div>
  )
}
