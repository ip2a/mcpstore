import { useMemo, useState } from "react"
import { EyeIcon, PlusIcon } from "lucide-react"

import { JsonBlock } from "@/components/shared/json-block"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { ServiceListCard } from "@/components/shared/service-list-card"
import { useAgentConfigQuery, useStoreConfigQuery } from "@/features/config/queries"
import { useScopesQuery } from "@/features/agents/queries"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import { AddScopeServiceDialog } from "@/features/agents/add-scope-service-dialog"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import type { AgentItem, ScopeSummary, ScopeView, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type ScopeCardId = "store" | string

const ALL_SCOPE_ID = "all"
const STORE_SCOPE_ID = "store"

function scopeIdFromSummary(summary: ScopeSummary): ScopeCardId {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "all") return t("filterAll")
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

export function AgentsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  loading: boolean
  busy: string | null
  onDeclareScope: (agentId: string, serviceName: string) => void
  onOpenService: (service: ServiceInstance) => void
  onRefresh: () => void
  onRemoveScope: (agentId: string, serviceName: string) => void
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<ScopeCardId>("all")
  const [selectedScopeId, setSelectedScopeId] = useState<ScopeCardId>("all")
  const [addScopeDialogOpen, setAddScopeDialogOpen] = useState(false)
  const [configScopeId, setConfigScopeId] = useState(ALL_SCOPE_ID)
  const [configDialogOpen, setConfigDialogOpen] = useState(false)
  const storeConfigQuery = useStoreConfigQuery()
  const configAgentId = configScopeId.startsWith("agent:") ? configScopeId.slice(6) : ""
  const agentConfigQuery = useAgentConfigQuery(configAgentId)
  const configValue =
    configScopeId === "store" ? storeConfigQuery.data : configAgentId ? agentConfigQuery.data : null
  const configLoading = storeConfigQuery.isFetching || agentConfigQuery.isFetching

  const scopesQuery = useScopesQuery()
  const scopes = scopesQuery.data || []

  const selectedScope: ScopeView = useMemo(
    () =>
      selectedScopeId === "all"
        ? { type: "all" }
        : selectedScopeId === STORE_SCOPE_ID
          ? { type: "store" }
          : { type: "agent", agent_id: selectedScopeId },
    [selectedScopeId],
  )

  const {
    activeAgentId,
    addableServiceNames,
    loadAgentScope,
    loadingScopeServices,
    loadingScopeTools,
    scopeServiceName,
    scopeServices,
    scopeServicesError,
    scopeServicesErrorMessage,
    scopeTools,
    scopeToolsError,
    scopeToolsErrorMessage,
    setScopeServiceName,
  } = useAgentScope({ busy: props.busy, selectedScope, services: props.services })

  const loadingScope = loadingScopeServices || loadingScopeTools
  const scopeError = scopeServicesError || scopeToolsError
  const scopeErrorMessage = scopeServicesError ? scopeServicesErrorMessage : scopeToolsErrorMessage

  const scopeToolCountByInstance = useMemo(() => {
    const counts = new Map<string, number>()
    for (const { instance } of scopeTools) {
      counts.set(instance.instance_id, (counts.get(instance.instance_id) || 0) + 1)
    }
    return counts
  }, [scopeTools])

  function loadScopeConfig() {
    void (configAgentId ? agentConfigQuery.refetch() : storeConfigQuery.refetch())
  }

  function selectScopeTab(scopeId: ScopeCardId) {
    setConfigScopeId(
      scopeId === "all" ? ALL_SCOPE_ID : scopeId === STORE_SCOPE_ID ? scopeId : `agent:${scopeId}`,
    )
    setSelectedScopeId(scopeId)
    setActiveTab(scopeId)
  }

  async function handleDeclareScope(agentId: string, serviceName: string) {
    await props.onDeclareScope(agentId, serviceName)
  }

  function handleRefresh() {
    void props.onRefresh()
    void loadAgentScope()
  }

  const scopeTabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col gap-4 overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("agentWorkspaceTitle")}</h1>}
        subtitle={t("agentWorkspaceDescription", { count: props.agents.length })}
      />

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        <Tabs
          value={activeTab}
          onValueChange={(value) => selectScopeTab(value)}
          className={WORKSPACE_TABS_CLASSNAME}
        >
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <WorkspaceTabsList value={activeTab}>
              {scopes.map((summary) => {
                const scopeId = scopeIdFromSummary(summary)
                const title = scopeTitleFromSummary(summary, t)
                return (
                  <TabsTrigger key={scopeId} value={scopeId} title={title} className="flex-none">
                    {title}
                  </TabsTrigger>
                )
              })}
            </WorkspaceTabsList>
            <ScopeActions
              className="ml-auto"
              busy={Boolean(props.busy)}
              canAddService={selectedScope.type === "agent" && addableServiceNames.length > 0}
              loading={props.loading || loadingScope}
              onAddService={() => setAddScopeDialogOpen(true)}
              onDetails={() => {
                loadScopeConfig()
                setConfigDialogOpen(true)
              }}
              onRefresh={handleRefresh}
            />
          </div>

          {scopes.map((summary) => {
            const scopeId = scopeIdFromSummary(summary)
            return (
              <TabsContent key={scopeId} value={scopeId} className={scopeTabContentClassName}>
                {activeTab === scopeId ? (
                  <ScopeDetailPanel
                    addScopeDialogOpen={addScopeDialogOpen}
                    addableServiceNames={addableServiceNames}
                    activeAgentId={activeAgentId}
                    busy={props.busy}
                    configDialogOpen={configDialogOpen}
                    configLoading={configLoading}
                    configValue={configValue}
                    loadingScopeServices={loadingScopeServices}
                    onDeclareScope={handleDeclareScope}
                    onOpenService={props.onOpenService}
                    scopeError={scopeError}
                    scopeErrorMessage={scopeErrorMessage}
                    scopeServiceName={scopeServiceName}
                    scopeServices={scopeServices}
                    scopeToolCountByInstance={scopeToolCountByInstance}
                    selectedScope={selectedScope}
                    setAddScopeDialogOpen={setAddScopeDialogOpen}
                    setConfigDialogOpen={setConfigDialogOpen}
                    setScopeServiceName={setScopeServiceName}
                    loadAgentScope={loadAgentScope}
                  />
                ) : null}
              </TabsContent>
            )
          })}
        </Tabs>
      </PanelCard>
    </>
  )
}

function ScopeActions({
  busy,
  canAddService,
  className,
  loading,
  onAddService,
  onDetails,
  onRefresh,
}: {
  busy: boolean
  canAddService: boolean
  className?: string
  loading: boolean
  onAddService: () => void
  onDetails: () => void
  onRefresh: () => void
}) {
  const { t } = useI18n()

  return (
    <div className={cn("flex shrink-0 gap-2", className)}>
      {canAddService ? (
        <Button size="sm" variant="outline" onClick={onAddService} disabled={busy}>
          <PlusIcon data-icon="inline-start" />
          {t("assignService")}
        </Button>
      ) : null}
      <Button size="sm" variant="outline" onClick={onDetails} disabled={loading}>
        <EyeIcon data-icon="inline-start" />
        {t("details")}
      </Button>
      <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
        <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
        {t("refresh")}
      </Button>
    </div>
  )
}

function ScopeDetailPanel({
  activeAgentId,
  addableServiceNames,
  addScopeDialogOpen,
  busy,
  configDialogOpen,
  configLoading,
  configValue,
  loadingScopeServices,
  loadAgentScope,
  onDeclareScope,
  onOpenService,
  scopeError,
  scopeErrorMessage,
  scopeServiceName,
  scopeServices,
  scopeToolCountByInstance,
  selectedScope,
  setAddScopeDialogOpen,
  setConfigDialogOpen,
  setScopeServiceName,
}: {
  activeAgentId: string
  addableServiceNames: string[]
  addScopeDialogOpen: boolean
  busy: string | null
  configDialogOpen: boolean
  configLoading: boolean
  configValue: unknown
  loadingScopeServices: boolean
  loadAgentScope: () => void
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
  onOpenService: (service: ServiceInstance) => void
  scopeError: unknown
  scopeErrorMessage: string
  scopeServiceName: string
  scopeServices: ServiceInstance[]
  scopeToolCountByInstance: Map<string, number>
  selectedScope: ScopeView
  setAddScopeDialogOpen: (open: boolean) => void
  setConfigDialogOpen: (open: boolean) => void
  setScopeServiceName: (name: string) => void
}) {
  const { t } = useI18n()

  return (
    <>
      <Dialog open={configDialogOpen} onOpenChange={setConfigDialogOpen}>
        <DialogContent className="flex max-h-[min(85vh,720px)] flex-col gap-4 overflow-hidden sm:max-w-2xl">
          <DialogHeader className="shrink-0">
            <DialogTitle>{t("configuration")}</DialogTitle>
          </DialogHeader>
          <div className="min-h-0 shrink overflow-hidden">
            {configLoading && !configValue ? (
              <PageSkeleton />
            ) : (
              <JsonBlock value={configValue || {}} className="h-[min(55vh,480px)] max-h-none" />
            )}
          </div>
        </DialogContent>
      </Dialog>

      <ScrollPane
        className="min-h-0 flex-1"
        innerClassName={cn(
          WORKSPACE_LIST_INSET_CLASSNAME,
          "grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4",
        )}
      >
        {scopeError ? (
          <PageError
            className="col-span-full"
            title={t("agentScopeFailedToLoad")}
            message={scopeErrorMessage}
            onRefresh={loadAgentScope}
          />
        ) : loadingScopeServices ? (
          <PageSkeleton className="col-span-full" />
        ) : scopeServices.length ? (
          scopeServices.map((service) => {
            const toolCount =
              scopeToolCountByInstance.get(service.instance_id) ?? service.tools?.length ?? 0

            return (
              <ServiceListCard
                key={service.instance_id}
                service={service}
                toolCount={toolCount}
                onSelect={() => onOpenService(service)}
              />
            )
          })
        ) : (
          <PageEmpty
            className="col-span-full"
            title={t("noServices")}
            description={t("noServicesDescription")}
            onRefresh={loadAgentScope}
          />
        )}
      </ScrollPane>

      {selectedScope.type === "agent" ? (
        <AddScopeServiceDialog
          addableServiceNames={addableServiceNames}
          agentId={activeAgentId}
          busy={busy}
          onDeclareScope={onDeclareScope}
          onOpenChange={setAddScopeDialogOpen}
          open={addScopeDialogOpen}
          scopeServiceName={scopeServiceName}
          setScopeServiceName={setScopeServiceName}
        />
      ) : null}
    </>
  )
}
