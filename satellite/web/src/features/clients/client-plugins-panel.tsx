import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { ScrollPane } from "@/components/shared/scroll-pane"
import { PluginListCard } from "@/components/shared/plugin-list-card"
import { Button } from "@/components/ui/button"
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
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"

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
          <Button size="sm" variant="outline" disabled={busy} onClick={() => onUpdate(clientId)}>
            {t("clientPluginUpdate")}
          </Button>
        ) : null}
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => onUninstall(clientId)}>
          {t("clientPluginUninstall")}
        </Button>
      </div>
    )
  }

  if (marketEntry && !marketEntry.installed) {
    return (
      <Button size="sm" variant="outline" disabled={busy} onClick={() => onInstall(clientId)}>
        {t("clientPluginInstall")}
      </Button>
    )
  }

  return null
}

export function useDetectMarketClients() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
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
}

/** Plugin market panel for the Clients → Market tab (card grid). */
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
  const marketPlugins = market.data?.plugins ?? []

  if (market.isError) {
    return (
      <div className="px-4 py-2">
        <p className="text-sm text-destructive">
          {market.error instanceof Error ? market.error.message : String(market.error)}
        </p>
      </div>
    )
  }

  if (marketPlugins.length === 0) {
    return (
      <div className="px-4 py-2">
        <p className="text-sm text-muted-foreground">{t("clientPluginMarketEmpty")}</p>
      </div>
    )
  }

  return (
    <ScrollPane
      className="min-h-0 flex-1"
      innerClassName="px-4 py-2 grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"
    >
      {marketPlugins.map((entry) => (
        <PluginListCard
          key={entry.id}
          busy={busy}
          plugin={{
            id: entry.id,
            name: entry.id,
            title: entry.label,
            description: entry.description?.trim() || "",
            version: entry.plugin_version,
            author: "mcpstore",
            installed: Boolean(entry.installed),
            kind: "agent",
            updateAvailable: entry.update_available,
          }}
          onInstall={onInstall}
          onUpdate={onUpdate}
        />
      ))}
    </ScrollPane>
  )
}
