import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import {
  detectMarketClients,
  disableClientPlugin,
  enableClientPlugin,
  installClientPlugin,
  listClientPlugins,
  listMarketPlugins,
  uninstallClientPlugin,
  type ClientPlugin,
  type MarketPlugin,
} from "@/lib/api/plugins"
import { queryKeys } from "@/lib/query-keys"
import { useI18n } from "@/lib/i18n-context"
import { Button } from "@/components/ui/button"

function usePluginRefresh() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
  }
}

/** Install/uninstall/enable/disable with toast + cache refresh. */
export function usePluginAction() {
  const { t } = useI18n()
  const refresh = usePluginRefresh()
  return useMutation({
    mutationFn: async ({
      action,
      id,
    }: {
      action: "install" | "uninstall" | "enable" | "disable"
      id: string
    }) => {
      if (action === "install") {
        await installClientPlugin(id)
        // Market install leaves the plugin disabled; enable so it appears in the clients list.
        await enableClientPlugin(id)
        return
      }
      if (action === "uninstall") return uninstallClientPlugin(id)
      if (action === "enable") return enableClientPlugin(id)
      return disableClientPlugin(id)
    },
    onSuccess: () => {
      toast.success(t("clientPluginActionDone"))
      refresh()
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : String(error))
    },
  })
}

/** Update = uninstall → install → enable. */
export function usePluginUpdate() {
  const { t } = useI18n()
  const refresh = usePluginRefresh()
  return useMutation({
    mutationFn: async (id: string) => {
      await uninstallClientPlugin(id)
      await installClientPlugin(id)
      await enableClientPlugin(id)
    },
    onSuccess: () => {
      toast.success(t("clientPluginActionDone"))
      refresh()
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : String(error))
    },
  })
}

export function useClientPluginsQuery() {
  return useQuery({
    queryKey: queryKeys.clientPlugins,
    queryFn: listClientPlugins,
  })
}

export function useClientPluginMarketQuery() {
  return useQuery({
    queryKey: queryKeys.clientPluginMarket,
    queryFn: listMarketPlugins,
  })
}

/** Install / Update / Uninstall controls for one client row, left of the Connect switch. */
export function ClientPluginRowActions({
  clientId,
  busy,
  installed,
  marketEntry,
  onInstall,
  onUninstall,
  onUpdate,
}: {
  clientId: string
  busy: boolean
  installed: ClientPlugin | undefined
  marketEntry: MarketPlugin | undefined
  onInstall: (id: string) => void
  onUninstall: (id: string) => void
  onUpdate: (id: string) => void
}) {
  const { t } = useI18n()

  if (installed) {
    return (
      <div className="flex shrink-0 items-center gap-1">
        {marketEntry?.update_available ? (
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => onUpdate(clientId)}
          >
            {t("clientPluginUpdate")}
          </Button>
        ) : null}
        <Button
          size="sm"
          variant="ghost"
          disabled={busy}
          onClick={() => onUninstall(clientId)}
        >
          {t("clientPluginUninstall")}
        </Button>
      </div>
    )
  }

  if (marketEntry && !marketEntry.installed) {
    return (
      <Button
        size="sm"
        variant="outline"
        disabled={busy}
        onClick={() => onInstall(clientId)}
      >
        {t("clientPluginInstall")}
      </Button>
    )
  }

  return null
}

/** Plugin market panel for the Clients → Market tab. */
export function ClientPluginMarketPanel({
  busy,
  onInstall,
  onUpdate,
}: {
  busy: boolean
  onInstall: (id: string) => void
  onUpdate: (id: string) => void
}) {
  const { t } = useI18n()
  const market = useClientPluginMarketQuery()
  const queryClient = useQueryClient()

  const detect = useMutation({
    mutationFn: detectMarketClients,
    onSuccess: ({ detected }) => {
      toast.success(t("clientPluginDetectDone", { count: detected.length }))
      void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
      void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : String(error))
    },
  })

  const marketPlugins = market.data?.plugins ?? []

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-b px-4 py-2">
        <Button
          size="sm"
          variant="outline"
          disabled={detect.isPending}
          onClick={() => detect.mutate()}
        >
          {detect.isPending ? t("clientPluginDetecting") : t("clientPluginDetect")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={market.isFetching}
          onClick={() => void market.refetch()}
        >
          {t("clientPluginMarketRefresh")}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-2">
        {market.isError ? (
          <p className="px-1 py-2 text-sm text-destructive">
            {market.error instanceof Error ? market.error.message : String(market.error)}
          </p>
        ) : marketPlugins.length === 0 ? (
          <p className="px-1 py-2 text-sm text-muted-foreground">{t("clientPluginMarketEmpty")}</p>
        ) : (
          <div className="divide-y divide-border/50">
            {marketPlugins.map((entry) => (
              <MarketRow
                key={entry.id}
                entry={entry}
                busy={busy}
                onInstall={() => onInstall(entry.id)}
                onUpdate={() => onUpdate(entry.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function MarketRow({
  entry,
  busy,
  onInstall,
  onUpdate,
}: {
  entry: MarketPlugin
  busy: boolean
  onInstall: () => void
  onUpdate: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">
          {entry.label}
          <span className="ml-2 text-xs font-normal text-muted-foreground">
            {entry.plugin_version}
          </span>
          {entry.update_available ? (
            <span className="ml-2 rounded bg-amber-500/15 px-1.5 py-0.5 text-xs text-amber-600">
              {t("clientPluginUpdateAvailable", { version: entry.plugin_version })}
            </span>
          ) : null}
        </p>
        {entry.description ? (
          <p className="truncate text-xs text-muted-foreground">{entry.description}</p>
        ) : null}
      </div>
      {entry.installed ? (
        <Button
          size="sm"
          variant="outline"
          className="w-20"
          disabled={busy || !entry.update_available}
          onClick={onUpdate}
        >
          {entry.update_available ? t("clientPluginUpdate") : t("clientPluginInstalled")}
        </Button>
      ) : (
        <Button size="sm" className="w-20" disabled={busy} onClick={onInstall}>
          {t("clientPluginInstall")}
        </Button>
      )}
    </div>
  )
}
