import { RefreshCwIcon } from "lucide-react"
import { useMemo, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"

import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { PluginListCard, type PluginItem } from "@/components/shared/plugin-list-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  useClientPluginMarketQuery,
  useClientPluginsQuery,
} from "@/features/clients/client-plugins-panel"
import { useScopeServicesQuery } from "@/features/agents/queries"
import { queryKeys } from "@/lib/query-keys"
import { useI18n } from "@/lib/i18n-context"

type PluginTab = "all" | "installed" | "not-installed" | "services" | "agents"

/** Curated third-party MCP services shown in the Services tab (no mock fillers). */
const SERVICE_CATALOG: Omit<PluginItem, "installed">[] = [
  {
    id: "filesystem",
    name: "filesystem",
    title: "Filesystem",
    description: "Read and write files on the local filesystem with configurable access controls.",
    version: "latest",
    author: "modelcontextprotocol",
    kind: "service",
  },
  {
    id: "github",
    name: "github",
    title: "GitHub",
    description: "Browse repositories, issues, and pull requests through the GitHub API.",
    version: "latest",
    author: "modelcontextprotocol",
    kind: "service",
  },
]

export function PluginsView() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [tab, setTab] = useState<PluginTab>("all")
  const [query, setQuery] = useState("")

  const servicesQuery = useScopeServicesQuery({ type: "store" })
  const marketQuery = useClientPluginMarketQuery()
  const pluginsQuery = useClientPluginsQuery()

  const tabs = useMemo(
    () => [
      { id: "all" as const, label: t("filterAll") },
      { id: "installed" as const, label: t("pluginInstalled") },
      { id: "not-installed" as const, label: t("pluginsTabNotInstalled") },
      { id: "services" as const, label: t("services") },
      { id: "agents" as const, label: t("agents") },
    ],
    [t],
  )

  const allPlugins = useMemo(() => {
    const installedNames = new Set(
      (servicesQuery.data ?? []).map((service) => service.service_name.toLowerCase()),
    )
    const servicePlugins: PluginItem[] = SERVICE_CATALOG.map((entry) => ({
      ...entry,
      installed: installedNames.has(entry.name.toLowerCase()),
    }))

    const installedById = new Map(
      (pluginsQuery.data?.plugins ?? []).map((plugin) => [plugin.id, plugin]),
    )
    const agentPlugins: PluginItem[] = (marketQuery.data?.plugins ?? []).map((entry) => {
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
      }
    })

    return [...servicePlugins, ...agentPlugins]
  }, [marketQuery.data, pluginsQuery.data, servicesQuery.data])

  const visiblePlugins = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return allPlugins.filter((plugin) => {
      if (tab === "installed" && !plugin.installed) return false
      if (tab === "not-installed" && plugin.installed) return false
      if (tab === "services" && plugin.kind !== "service") return false
      if (tab === "agents" && plugin.kind !== "agent") return false
      if (!normalized) return true
      return (
        plugin.name.toLowerCase().includes(normalized) ||
        plugin.title.toLowerCase().includes(normalized) ||
        plugin.description.toLowerCase().includes(normalized) ||
        plugin.author.toLowerCase().includes(normalized)
      )
    })
  }, [allPlugins, query, tab])

  const loading =
    (servicesQuery.isPending && !servicesQuery.data) ||
    (marketQuery.isPending && !marketQuery.data) ||
    (pluginsQuery.isPending && !pluginsQuery.data)
  const errored = servicesQuery.isError || marketQuery.isError || pluginsQuery.isError
  const refreshing =
    servicesQuery.isFetching || marketQuery.isFetching || pluginsQuery.isFetching

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.scopeServices({ type: "store" }) })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("pluginRegistryTitle")}</h1>}
        subtitle={t("pluginRegistryDescription", { count: visiblePlugins.length })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs value={tab} onValueChange={(value) => setTab(value as PluginTab)} className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              {tabs.map((item) => (
                <TabsTrigger key={item.id} value={item.id} title={item.label} className="flex-none">
                  {item.label}
                </TabsTrigger>
              ))}
            </TabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <CollapsibleSearchBox
                inline
                id="plugin-list-search"
                placeholder={t("searchPlugins")}
                value={query}
                onChange={setQuery}
              />
              <Button size="sm" variant="outline" onClick={refresh} disabled={refreshing}>
                <RefreshCwIcon data-icon="inline-start" />
                {t("refresh")}
              </Button>
            </div>
          </div>

          {tabs.map((item) => (
            <TabsContent key={item.id} value={item.id} className={tabContentClassName}>
              {tab === item.id ? (
                errored ? (
                  <div className="px-4 py-2">
                    <PageError onRefresh={refresh} />
                  </div>
                ) : loading ? (
                  <div className="px-4 py-2">
                    <PageSkeleton />
                  </div>
                ) : visiblePlugins.length ? (
                  <ScrollPane
                    className="min-h-0 flex-1"
                    innerClassName="px-4 py-2 grid auto-rows-fr grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-3"
                  >
                    {visiblePlugins.map((plugin) => (
                      <PluginListCard key={`${plugin.kind}:${plugin.id}`} plugin={plugin} />
                    ))}
                  </ScrollPane>
                ) : (
                  <div className="px-4 py-2">
                    <PageEmpty title={t("noPlugins")} description={t("noPluginsDescription")} onRefresh={refresh} />
                  </div>
                )
              ) : null}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>
    </>
  )
}
