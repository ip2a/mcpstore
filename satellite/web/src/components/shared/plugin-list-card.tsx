import { PuzzleIcon } from "lucide-react"

import { clientIconUrls } from "@/assets/icons/clients"
import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
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
    <BasicCard
      className={className}
      icon={
        iconUrl ? (
          <img
            src={iconUrl}
            alt=""
            className={cn(basicCardIconClassName, "object-contain")}
            aria-hidden
          />
        ) : (
          <PuzzleIcon className={basicCardIconClassName} aria-hidden />
        )
      }
      title={displayName}
      subtitle={
        <>
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
        </>
      }
      action={
        plugin.installed ? (
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
        )
      }
      body={<p className="line-clamp-2">{description || t("noDescription")}</p>}
      footer={
        plugin.version ? (
          <p className="truncate font-mono tabular-nums">{plugin.version}</p>
        ) : undefined
      }
    />
  )
}
