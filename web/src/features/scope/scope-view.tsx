import { useState, useMemo } from "react"
import { PencilIcon, RefreshCwIcon, ChevronRightIcon } from "lucide-react"

import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { useScopesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import { ToolConfigRow } from "@/features/scope/tool-config-row"
import type { ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

function scopeIdFromSummary(summary: ScopeSummary): string {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "root") return "root"
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

export function ScopeView(props: { services: ServiceInstance[] }) {
  const { t } = useI18n()
  const scopesQuery = useScopesQuery()
  const scopes = scopesQuery.data || []
  const [selectedScopeId, setSelectedScopeId] = useState<string>("root")

  const selectedScope: ScopeView = useMemo(() => {
    if (selectedScopeId === "root") return { type: "root" }
    if (selectedScopeId === "store") return { type: "store" }
    return { type: "agent", agent_id: selectedScopeId }
  }, [selectedScopeId])

  const scopeServices = useMemo(() => {
    if (selectedScope.type === "root") return props.services
    if (selectedScope.type === "store") {
      return props.services.filter((service) => service.scope.type === "store")
    }
    return props.services.filter(
      (service) =>
        service.scope.type === "agent" && service.scope.agent_id === selectedScope.agent_id,
    )
  }, [props.services, selectedScope])

  const toolQueries = useInstanceToolsQueries(scopeServices, "all")
  const loading = scopesQuery.isFetching || toolQueries.some((result) => result.isFetching)
  const error = scopesQuery.error || toolQueries.find((result) => result.error)?.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : "Failed to load scope"

  const serviceTools = useMemo(
    () =>
      scopeServices.map((service, index) => ({
        service,
        tools: toolQueries[index]?.data || [],
      })),
    [scopeServices, toolQueries],
  )

  async function handleRefresh() {
    await scopesQuery.refetch()
    await Promise.all(toolQueries.map((result) => result.refetch()))
  }

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("scopeConfigurationTitle")}</h1>}
        subtitle={t("scopeConfigurationDescription")}
      />

      <PanelCard className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex min-w-0 shrink-0 items-center gap-3 border-b px-4 py-3">
          <Tabs value={selectedScopeId} onValueChange={setSelectedScopeId} className="flex-1">
            <TabsList variant="line" className="h-9 w-fit">
              {scopes.map((summary) => {
                const scopeId = scopeIdFromSummary(summary)
                const title = scopeTitleFromSummary(summary, t)
                return (
                  <TabsTrigger key={scopeId} value={scopeId} className="flex-none">
                    {title}
                  </TabsTrigger>
                )
              })}
            </TabsList>
          </Tabs>
          <Button size="sm" variant="outline" onClick={handleRefresh} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </div>

        <ScrollPane className="min-h-0 flex-1">
          {error ? (
            <PageError title={t("scopeFailedToLoad")} message={errorMessage} onRefresh={handleRefresh} />
          ) : loading && !serviceTools.length ? (
            <PageSkeleton />
          ) : !serviceTools.length ? (
            <div className="flex h-full items-center justify-center p-8 text-center text-muted-foreground">
              {t("noServices")}
            </div>
          ) : (
            <div className="space-y-3 p-4">
              {serviceTools.map(({ service, tools }) => (
                <ServiceToolGroup
                  key={service.instance_id}
                  service={service}
                  tools={tools}
                  scope={selectedScope}
                />
              ))}
            </div>
          )}
        </ScrollPane>
      </PanelCard>
    </>
  )
}

function ServiceToolGroup({
  service,
  tools,
  scope,
}: {
  service: ServiceInstance
  tools: Array<{ name: string; description: string; title?: string | null }>
  scope: ScopeView
}) {
  const [open, setOpen] = useState(false)

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="rounded-lg border border-border/50 bg-card">
        <CollapsibleTrigger asChild>
          <button
            type="button"
            className="flex w-full items-center gap-2 px-4 py-3 text-left transition-colors hover:bg-muted/40"
          >
            <ChevronRightIcon
              className={cn("size-4 shrink-0 transition-transform", open && "rotate-90")}
            />
            <span className="flex-1 font-medium">{service.service_name}</span>
            <span className="text-sm text-muted-foreground">{tools.length} tools</span>
          </button>
        </CollapsibleTrigger>

        <CollapsibleContent>
          <div className="border-t border-border/50">
            {tools.map((tool) => (
              <ToolConfigRow
                key={tool.name}
                service={service}
                tool={tool}
                scope={scope}
              />
            ))}
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  )
}
