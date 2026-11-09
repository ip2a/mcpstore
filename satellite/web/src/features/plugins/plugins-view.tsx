import { useEffect, useMemo, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"

import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { PluginListCard, type PluginItem } from "@/components/shared/plugin-list-card"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import {
  useClientPluginMarketQuery,
  useClientPluginsQuery,
  usePluginAction,
  usePluginUpdate,
} from "@/features/clients/client-plugins-panel"
import { ServicesMarketTab } from "@/features/plugins/services-market-tab"
import { queryKeys } from "@/lib/query-keys"
import { useI18n } from "@/lib/i18n-context"

type PluginTab = "installed" | "services" | "agents"

export function PluginsView({ initialTab }: { initialTab?: PluginTab } = {}) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [tab, setTab] = useState<PluginTab>(initialTab ?? "installed")
  const [query, setQuery] = useState("")
  const pluginAction = usePluginAction()
  const pluginUpdate = usePluginUpdate()

  useEffect(() => {
    if (initialTab) setTab(initialTab)
  }, [initialTab])

  const marketQuery = useClientPluginMarketQuery()
  const pluginsQuery = useClientPluginsQuery()
  const pluginBusy = pluginAction.isPending || pluginUpdate.isPending

  const tabs = useMemo(
    () => [
      { id: "installed" as const, label: t("pluginInstalled") },
      { id: "services" as const, label: t("services") },
      { id: "agents" as const, label: t("agents") },
    ],
    [t],
  )

  const agentPlugins = useMemo<PluginItem[]>(() => {
    const installedById = new Map(
      (pluginsQuery.data?.plugins ?? []).map((plugin) => [plugin.id, plugin]),
    )
    return (marketQuery.data?.plugins ?? []).map((entry) => {
      const installed = installedById.get(entry.id)
      return {
        id: entry.id,
        name: entry.id,
        title: entry.label,
        description: entry.description?.trim() || "",
        version: installed?.plugin_version || entry.plugin_version,
        author: "mcpstore",
        installed: Boolean(entry.installed || installed),
        kind: "agent" as const,
        updateAvailable: entry.update_available,
      }
    })
  }, [marketQuery.data, pluginsQuery.data])

  const visiblePlugins = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return agentPlugins.filter((plugin) => {
      if (tab === "installed" && !plugin.installed) return false
      if (!normalized) return true
      return (
        plugin.name.toLowerCase().includes(normalized) ||
        plugin.title.toLowerCase().includes(normalized) ||
        plugin.description.toLowerCase().includes(normalized) ||
        plugin.author.toLowerCase().includes(normalized)
      )
    })
  }, [agentPlugins, query, tab])

  const agentsLoading =
    (marketQuery.isPending && !marketQuery.data) || (pluginsQuery.isPending && !pluginsQuery.data)
  const agentsErrored = marketQuery.isError || pluginsQuery.isError
  const refreshing = marketQuery.isFetching || pluginsQuery.isFetching

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
    // The featured key is a prefix of every registry page key: one invalidation
    // covers both.
    void queryClient.invalidateQueries({ queryKey: queryKeys.servicesMarketFeatured })
    void queryClient.invalidateQueries({ queryKey: queryKeys.scopeServices({ type: "store" }) })
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("pluginRegistryTitle")}</h1>}
        subtitle={t("pluginRegistryDescription", { count: visiblePlugins.length })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs
          value={tab}
          onValueChange={(value) => setTab(value as PluginTab)}
          className={WORKSPACE_TABS_CLASSNAME}
        >
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <WorkspaceTabsList value={tab}>
              {tabs.map((item) => (
                <TabsTrigger key={item.id} value={item.id} title={item.label} className="flex-none">
                  {item.label}
                </TabsTrigger>
              ))}
            </WorkspaceTabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <UiverseSearchInput
                id="plugin-list-search"
                placeholder={t("searchPlugins")}
                value={query}
                onChange={setQuery}
              />
              <Button size="sm" variant="outline" onClick={refresh} disabled={refreshing}>
                <RefreshCwSpinIcon data-icon="inline-start" spinning={refreshing} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          <TabsContent value="services" className={tabContentClassName}>
            {tab !== "services" ? null : <ServicesMarketTab query={query} />}
          </TabsContent>

          {(["installed", "agents"] as const).map((item) => (
            <TabsContent key={item} value={item} className={tabContentClassName}>
              {tab !== item ? null : agentsErrored ? (
                <div className={WORKSPACE_LIST_INSET_CLASSNAME}>
                  <PageError onRefresh={refresh} />
                </div>
              ) : agentsLoading ? (
                <div className={WORKSPACE_LIST_INSET_CLASSNAME}>
                  <PageSkeleton />
                </div>
              ) : visiblePlugins.length ? (
                <ScrollPane
                  className="min-h-0 flex-1"
                  innerClassName={`${WORKSPACE_LIST_INSET_CLASSNAME} grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4`}
                >
                  {visiblePlugins.map((plugin) => (
                    <PluginListCard
                      key={`${plugin.kind}:${plugin.id}`}
                      busy={pluginBusy}
                      plugin={plugin}
                      onInstall={(id) => pluginAction.mutate({ action: "install", id })}
                      onUpdate={(id) => pluginUpdate.mutate(id)}
                    />
                  ))}
                </ScrollPane>
              ) : (
                <div className={WORKSPACE_LIST_INSET_CLASSNAME}>
                  <PageEmpty
                    title={t("noPlugins")}
                    description={t("noPluginsDescription")}
                    onRefresh={refresh}
                  />
                </div>
              )}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}
