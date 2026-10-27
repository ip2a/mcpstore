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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Switch } from "@/components/ui/switch"

/** One mutation shared by every plugin action: fire, toast, refresh state. */
function usePluginAction() {
  const queryClient = useQueryClient()
  const { t } = useI18n()
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
  }
  return useMutation({
    mutationFn: ({ action, id }: { action: "install" | "uninstall" | "enable" | "disable"; id: string }) => {
      const calls = {
        install: () => installClientPlugin(id),
        uninstall: () => uninstallClientPlugin(id),
        enable: () => enableClientPlugin(id),
        disable: () => disableClientPlugin(id),
      } as const
      return calls[action]()
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

/** Update = uninstall then install from the market, as one chained action. */
function usePluginUpdate() {
  const queryClient = useQueryClient()
  const { t } = useI18n()
  return useMutation({
    mutationFn: async (id: string) => {
      await uninstallClientPlugin(id)
      await installClientPlugin(id)
    },
    onSuccess: () => {
      toast.success(t("clientPluginActionDone"))
      void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
      void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
      void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : String(error))
    },
  })
}

export function ClientPluginsDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const action = usePluginAction()
  const update = usePluginUpdate()
  const busy = action.isPending || update.isPending

  const plugins = useQuery({
    queryKey: queryKeys.clientPlugins,
    queryFn: listClientPlugins,
    enabled: open,
  })

  const market = useQuery({
    queryKey: queryKeys.clientPluginMarket,
    queryFn: listMarketPlugins,
    enabled: open,
  })

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

  const installed = plugins.data?.plugins ?? []
  const marketPlugins = market.data?.plugins ?? []

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[80vh] overflow-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("clientPluginsTitle")}</DialogTitle>
          <DialogDescription>{t("clientPluginsDescription")}</DialogDescription>
        </DialogHeader>

        <section className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">{t("clientPluginsInstalledTitle")}</h3>
          {installed.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("clientPluginsNone")}</p>
          ) : (
            installed.map((plugin) => (
              <ClientPluginRow
                key={plugin.id}
                plugin={plugin}
                busy={busy}
                action={action.mutate}
              />
            ))
          )}
        </section>

        <section className="flex flex-col gap-1 border-t pt-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium">{t("clientPluginMarketTitle")}</h3>
            <div className="flex gap-2">
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
          </div>
          {market.isError ? (
            <p className="text-sm text-destructive">
              {market.error instanceof Error ? market.error.message : String(market.error)}
            </p>
          ) : marketPlugins.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("clientPluginMarketEmpty")}</p>
          ) : (
            marketPlugins.map((entry) => (
              <MarketRow
                key={entry.id}
                entry={entry}
                busy={busy}
                action={action.mutate}
              />
            ))
          )}
        </section>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function ClientPluginRow({
  plugin,
  busy,
  action,
}: {
  plugin: ClientPlugin
  busy: boolean
  action: (variables: { action: "install" | "uninstall" | "enable" | "disable"; id: string }) => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 rounded-md px-2 py-1.5 hover:bg-muted/50">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">
          {plugin.label}
          <span className="ml-2 text-xs font-normal text-muted-foreground">
            {plugin.plugin_version}
          </span>
        </p>
        {plugin.error ? (
          <p className="truncate text-xs text-destructive" title={plugin.error}>
            {plugin.error}
          </p>
        ) : null}
      </div>
      <Switch
        checked={plugin.enabled}
        disabled={busy || plugin.error != null}
        onCheckedChange={(checked) => action({ action: checked ? "enable" : "disable", id: plugin.id })}
      />
      <Button
        size="sm"
        variant="ghost"
        disabled={busy}
        onClick={() => action({ action: "uninstall", id: plugin.id })}
      >
        {t("clientPluginUninstall")}
      </Button>
    </div>
  )
}

function MarketRow({
  entry,
  busy,
  action,
}: {
  entry: MarketPlugin
  busy: boolean
  action: (variables: { action: "install" | "uninstall" | "enable" | "disable"; id: string }) => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 rounded-md px-2 py-1.5 hover:bg-muted/50">
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
          disabled={busy || !entry.update_available}
          onClick={() => update.mutate(entry.id)}
        >
          {entry.update_available ? t("clientPluginUpdate") : t("clientPluginInstalled")}
        </Button>
      ) : (
        <Button size="sm" disabled={busy} onClick={() => action({ action: "install", id: entry.id })}>
          {t("clientPluginInstall")}
        </Button>
      )}
    </div>
  )
}
