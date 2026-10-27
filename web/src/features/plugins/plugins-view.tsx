import { RefreshCwIcon } from "lucide-react"
import { useMemo, useState } from "react"

import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { PageEmpty } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { PluginListCard, type PluginItem } from "@/components/shared/plugin-list-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"

type PluginTab = "all" | "installed" | "not-installed"

const PLACEHOLDER_PLUGINS: PluginItem[] = [
  {
    id: "filesystem",
    name: "filesystem",
    title: "Filesystem",
    description: "Read and write files on the local filesystem with configurable access controls.",
    version: "1.2.0",
    author: "mcpstore",
    installed: true,
  },
  {
    id: "github",
    name: "github",
    title: "GitHub",
    description: "Browse repositories, issues, and pull requests through the GitHub API.",
    version: "0.9.1",
    author: "community",
    installed: true,
  },
  {
    id: "postgres",
    name: "postgres",
    title: "PostgreSQL",
    description: "Run read-only SQL queries against a PostgreSQL database.",
    version: "2.0.3",
    author: "community",
    installed: false,
  },
  {
    id: "brave-search",
    name: "brave-search",
    title: "Brave Search",
    description: "Web search powered by the Brave Search API.",
    version: "1.0.0",
    author: "community",
    installed: false,
  },
  {
    id: "slack",
    name: "slack",
    title: "Slack",
    description: "Send messages and read channel history from Slack workspaces.",
    version: "0.4.2",
    author: "community",
    installed: false,
  },
  {
    id: "memory",
    name: "memory",
    title: "Memory",
    description: "Persistent key-value memory store for agent sessions.",
    version: "1.1.0",
    author: "mcpstore",
    installed: true,
  },
]

export function PluginsView() {
  const { t } = useI18n()
  const [tab, setTab] = useState<PluginTab>("all")
  const [query, setQuery] = useState("")
  const [refreshing, setRefreshing] = useState(false)

  const tabs = useMemo(
    () => [
      { id: "all" as const, label: t("filterAll") },
      { id: "installed" as const, label: t("pluginInstalled") },
      { id: "not-installed" as const, label: t("pluginsTabNotInstalled") },
    ],
    [t],
  )

  const visiblePlugins = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return PLACEHOLDER_PLUGINS.filter((plugin) => {
      if (tab === "installed" && !plugin.installed) return false
      if (tab === "not-installed" && plugin.installed) return false
      if (!normalized) return true
      return (
        plugin.name.toLowerCase().includes(normalized) ||
        plugin.title.toLowerCase().includes(normalized) ||
        plugin.description.toLowerCase().includes(normalized) ||
        plugin.author.toLowerCase().includes(normalized)
      )
    })
  }, [query, tab])

  const refresh = () => {
    setRefreshing(true)
    window.setTimeout(() => setRefreshing(false), 400)
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
                visiblePlugins.length ? (
                  <ScrollPane
                    className="min-h-0 flex-1"
                    innerClassName="px-4 py-2 grid auto-rows-fr grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-3"
                  >
                    {visiblePlugins.map((plugin) => (
                      <PluginListCard key={plugin.id} plugin={plugin} />
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
