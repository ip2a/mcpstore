import { useEffect, useMemo, useState } from "react"
import { EyeIcon, LinkIcon, PlusIcon, RefreshCwIcon, UnlinkIcon } from "lucide-react"

import { EntityRow } from "@/components/shared/entity-row"
import { JsonBlock } from "@/components/shared/json-block"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { useAgentConfigQuery, useStoreConfigQuery } from "@/features/config/queries"
import { ClientConfigPanel } from "@/features/config/client-config-panel"
import { useScopesQuery } from "@/features/agents/queries"
import { ServiceRowMeta } from "@/components/shared/service-row-meta"
import { ServiceConnectionMark, ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { AddScopeServiceDialog } from "@/features/agents/add-scope-service-dialog"
import { getAgentId } from "@/features/agents/model"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { isServiceConnected } from "@/features/services/service-display-status"
import { type AgentItem, type ScopeSummary, type ScopeView, type ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { getServiceEndpointLabel } from "@/lib/service-info"
import { cn } from "@/lib/utils"

type RightPaneView = "overview" | "scope"
type ScopeCardId = "store" | string
type ScopeConnectionStats = { services: number; connected: number; disconnected: number }

const ALL_SCOPE_ID = "all"
const STORE_SCOPE_ID = "store"

function connectionStatsForServices(services: ServiceInstance[]): ScopeConnectionStats {
  let connected = 0
  for (const service of services) {
    if (isServiceConnected(service.state)) connected += 1
  }
  return {
    services: services.length,
    connected,
    disconnected: services.length - connected,
  }
}

function scopeStatsFromServices(services: ServiceInstance[]) {
  const storeServices = services.filter((service) => service.scope.type === "store")
  const agents = new Map<string, ScopeConnectionStats>()
  for (const service of services) {
    if (service.scope.type !== "agent") continue
    const agentId = service.scope.agent_id
    const current = agents.get(agentId) || { services: 0, connected: 0, disconnected: 0 }
    current.services += 1
    if (isServiceConnected(service.state)) current.connected += 1
    else current.disconnected += 1
    agents.set(agentId, current)
  }
  return {
    root: connectionStatsForServices(services),
    store: connectionStatsForServices(storeServices),
    agents,
  }
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
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("overview")
  const [selectedScopeId, setSelectedScopeId] = useState<ScopeCardId>(STORE_SCOPE_ID)
  const [addScopeDialogOpen, setAddScopeDialogOpen] = useState(false)
  const [previewServiceId, setPreviewServiceId] = useState<string | null>(null)
  const [configScopeId, setConfigScopeId] = useState(ALL_SCOPE_ID)
  const [configDialogOpen, setConfigDialogOpen] = useState(false)
  const [mcpHubDialogOpen, setMcpHubDialogOpen] = useState(false)
  const storeConfigQuery = useStoreConfigQuery()
  const configAgentId = configScopeId.startsWith("agent:") ? configScopeId.slice(6) : ""
  const agentConfigQuery = useAgentConfigQuery(configAgentId)
  const configValue = configScopeId === "store" ? storeConfigQuery.data : configAgentId ? agentConfigQuery.data : null
  const configLoading = storeConfigQuery.isFetching || agentConfigQuery.isFetching

  const scopesQuery = useScopesQuery()
  const scopes = scopesQuery.data || []

  const selectedScope: ScopeView = useMemo(
    () =>
      selectedScopeId === "root"
        ? { type: "root" }
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

  const storeServices = useMemo(
    () => props.services.filter((service) => service.scope.type === "store"),
    [props.services],
  )

  const scopeStats = useMemo(() => scopeStatsFromServices(props.services), [props.services])

  const loadingScope = loadingScopeServices || loadingScopeTools
  const scopeError = scopeServicesError || scopeToolsError
  const scopeErrorMessage = scopeServicesError ? scopeServicesErrorMessage : scopeToolsErrorMessage
  const scopeTitle = selectedScopeId === "root"
    ? t("global")
    : selectedScopeId === STORE_SCOPE_ID
      ? t("store")
      : selectedScopeId

  const previewService = useMemo(
    () => scopeServices.find((service) => service.instance_id === previewServiceId) || null,
    [previewServiceId, scopeServices],
  )

  const previewToolCount = useMemo(() => {
    if (!previewService) return 0
    return scopeTools.filter(({ instance }) => instance.instance_id === previewService.instance_id).length
  }, [previewService, scopeTools])

  const scopeToolCountByInstance = useMemo(() => {
    const counts = new Map<string, number>()
    for (const { instance } of scopeTools) {
      counts.set(instance.instance_id, (counts.get(instance.instance_id) || 0) + 1)
    }
    return counts
  }, [scopeTools])

  useEffect(() => {
    setPreviewServiceId(null)
  }, [selectedScopeId, rightPaneView])

  function loadScopeConfig() {
    void (configAgentId ? agentConfigQuery.refetch() : storeConfigQuery.refetch())
  }

  const workspaceStats = useMemo(() => {
    const agentServiceCount = props.agents.reduce((sum, agent) => sum + agent.instance_ids.length, 0)
    return {
      scopeCount: 1 + props.agents.length,
      serviceCount: storeServices.length + agentServiceCount,
      catalogCount: props.services.length,
    }
  }, [props.agents, props.services.length, storeServices.length])

  function showWorkspaceOverview() {
    setConfigScopeId(ALL_SCOPE_ID)
    setRightPaneView("overview")
  }

  function selectScope(scopeId: ScopeCardId) {
    setConfigScopeId(
      scopeId === "root"
        ? ALL_SCOPE_ID
        : scopeId === STORE_SCOPE_ID
          ? scopeId
          : `agent:${scopeId}`,
    )
    setSelectedScopeId(scopeId)
    setRightPaneView("scope")
  }

  function scopeCardMeta(summary: ScopeSummary) {
    const stats =
      summary.scope.type === "root"
        ? scopeStats.root
        : summary.scope.type === "store"
          ? scopeStats.store
          : scopeStats.agents.get(summary.scope.agent_id) || {
              services: 0,
              connected: 0,
              disconnected: 0,
            }
    return t("agentServicesConnectionCount", {
      services: stats.services,
      connected: stats.connected,
    })
  }

  async function handleDeclareScope(agentId: string, serviceName: string) {
    await props.onDeclareScope(agentId, serviceName)
  }

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("scope")}</p>
            <button
              type="button"
              onClick={showWorkspaceOverview}
              className={cn(
                "mt-1 block max-w-full cursor-pointer truncate border-0 bg-transparent p-0 text-left text-lg font-semibold underline-offset-4 outline-none transition-opacity",
                "hover:underline active:opacity-70",
                rightPaneView === "overview" && "underline",
              )}
              title={t("agentWorkspaceTitle")}
            >
              {t("agentWorkspaceTitle")}
            </button>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("agentWorkspaceDescription", { count: props.agents.length })}
            </p>
          </div>
        </section>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3">
          {props.loading ? (
            <PageSkeleton />
          ) : (
            <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
              {scopes.map((summary) => {
                const scopeId =
                  summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
                const title =
                  summary.scope.type === "root"
                    ? t("global")
                    : summary.scope.type === "store"
                      ? t("store")
                      : summary.scope.type === "agent"
                        ? summary.scope.agent_id
                        : scopeId
                return (
                  <SelectableRowButton
                    key={scopeId}
                    meta={scopeCardMeta(summary)}
                    onClick={() => selectScope(scopeId)}
                    selected={scopeId === selectedScopeId && rightPaneView === "scope"}
                    title={title}
                  />
                )
              })}
            </ScrollPane>
          )}
        </div>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        {rightPaneView === "overview" ? (
          <>
            <WorkspacePreviewHeader
              loading={props.loading}
              onRefresh={() => {
                void props.onRefresh()
                void loadAgentScope()
              }}
            />

            <MetricGrid columns="four">
              <MetricTile
                variant="compact"
                label={t("scope")}
                value={String(workspaceStats.scopeCount)}
                title={String(workspaceStats.scopeCount)}
                hint={t("registeredAgentScopes")}
              />
              <MetricTile
                variant="compact"
                label={t("services")}
                value={String(workspaceStats.serviceCount)}
                hint={t("inScope", { count: workspaceStats.serviceCount })}
              />
              <MetricTile
                variant="compact"
                label={t("catalog")}
                value={String(workspaceStats.catalogCount)}
                hint={t("allServices")}
              />
              <MetricTile
                variant="compact"
                label={t("store")}
                value={String(storeServices.length)}
                hint={t("storeScope")}
              />
            </MetricGrid>

            <ScrollPane className="flex-1">
              <AgentWorkspaceOverview agents={props.agents} services={props.services} storeServices={storeServices} />
            </ScrollPane>
          </>
        ) : (
          <>
            <AgentPreviewHeader
              busy={Boolean(props.busy)}
              canAddService={selectedScope.type === "agent" && addableServiceNames.length > 0}
              loading={props.loading || loadingScope}
              onAddService={() => setAddScopeDialogOpen(true)}
              onDetails={() => {
                loadScopeConfig()
                setConfigDialogOpen(true)
              }}
              onHub={() => setMcpHubDialogOpen(true)}
              onRefresh={() => {
                void props.onRefresh()
                void loadAgentScope()
              }}
              scopeTitle={scopeTitle}
            />

            <Dialog open={configDialogOpen} onOpenChange={setConfigDialogOpen}>
              <DialogContent className="flex max-h-[min(85vh,720px)] flex-col gap-4 overflow-hidden sm:max-w-2xl">
                <DialogHeader className="shrink-0">
                  <DialogTitle>{t("configuration")}</DialogTitle>
                </DialogHeader>
                <div className="min-h-0 shrink overflow-hidden">
                  {configLoading && !configValue ? <PageSkeleton /> : <JsonBlock value={configValue || {}} className="h-[min(55vh,480px)] max-h-none" />}
                </div>
              </DialogContent>
            </Dialog>

            <ScrollPane className="min-h-0 flex-1" innerClassName="flex flex-col gap-4">
            {scopeError ? (
              <PageError
                title={t("agentScopeFailedToLoad")}
                message={scopeErrorMessage}
                onRefresh={loadAgentScope}
              />
            ) : loadingScopeServices ? (
              <PageSkeleton />
            ) : (
              <div className="flex flex-col gap-3">
                {scopeServices.length ? (
                    <div className="border-t">
                      {scopeServices.map((service) => {
                        const selected = service.instance_id === previewServiceId
                        const toolCount =
                          scopeToolCountByInstance.get(service.instance_id) ?? service.tools?.length ?? 0

                        return (
                          <EntityRow
                            key={service.instance_id}
                            variant="inline"
                            selected={selected}
                            className="min-h-14 cursor-pointer py-2.5 hover:bg-muted/60"
                            tabIndex={0}
                            onClick={() => setPreviewServiceId(service.instance_id)}
                            onKeyDown={(event) => {
                              if (event.target !== event.currentTarget) return
                              if (event.key === "Enter" || event.key === " ") {
                                event.preventDefault()
                                setPreviewServiceId(service.instance_id)
                              }
                            }}
                          >
                            <div className="min-w-0">
                              <div className="flex min-w-0 flex-nowrap items-center gap-x-2 text-base">
                                <span className="block min-w-0 truncate font-semibold">{service.service_name}</span>
                                <ServiceConnectionMark state={service.state} />
                                <span className="block min-w-0 truncate text-sm text-muted-foreground" title={getServiceEndpointLabel(service)}>
                                  {getServiceEndpointLabel(service)}
                                </span>
                              </div>
                              <ServiceRowMeta service={service} toolCount={toolCount} />
                            </div>
                          </EntityRow>
                        )
                      })}
                    </div>
                ) : (
                  <PageEmpty
                    title={t("noServices")}
                    description={t("noServicesDescription")}
                    onRefresh={loadAgentScope}
                  />
                )}
              </div>
            )}

            {(selectedScope.type === "store" || selectedScope.type === "agent") && (
              <ClientConfigPanel scope={selectedScope} open={mcpHubDialogOpen} onOpenChange={setMcpHubDialogOpen} />
            )}
            </ScrollPane>

            {selectedScope.type === "agent" ? (
              <AddScopeServiceDialog
                addableServiceNames={addableServiceNames}
                agentId={activeAgentId}
                busy={props.busy}
                onDeclareScope={handleDeclareScope}
                onOpenChange={setAddScopeDialogOpen}
                open={addScopeDialogOpen}
                scopeServiceName={scopeServiceName}
                setScopeServiceName={setScopeServiceName}
              />
            ) : null}

            <ScopeServiceSheet
              busy={Boolean(props.busy)}
              onOpenChange={(open) => {
                if (!open) setPreviewServiceId(null)
              }}
              onOpenService={(service) => {
                setPreviewServiceId(null)
                props.onOpenService(service)
              }}
              onRemoveScope={
                selectedScope.type === "agent" && activeAgentId
                  ? (serviceName) => {
                      setPreviewServiceId(null)
                      props.onRemoveScope(activeAgentId, serviceName)
                    }
                  : undefined
              }
              open={Boolean(previewService)}
              service={previewService}
              toolCount={previewToolCount}
            />
          </>
        )}
      </PanelCard>
    </TwoPanePage>
  )
}

function WorkspacePreviewHeader({ loading, onRefresh }: { loading: boolean; onRefresh: () => void }) {
  const { t } = useI18n()

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b pb-2">
      <strong className="truncate font-mono text-sm font-medium" title={t("agentWorkspaceTitle")}>
        {t("agentWorkspaceTitle")}
      </strong>
      <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
        <RefreshCwIcon data-icon="inline-start" />
        {t("refresh")}
      </Button>
    </div>
  )
}

function AgentWorkspaceOverview({
  agents,
  services,
  storeServices,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
  storeServices: ServiceInstance[]
}) {
  const { t } = useI18n()

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("storeScope")} titleAs="h2" className="border-b-0 pb-3" />
        <p className="break-words font-mono text-sm text-muted-foreground">
          {storeServices.length
            ? storeServices.map((service) => service.service_name).join(" · ")
            : t("noServices")}
        </p>
      </section>
      <section className="border-b pb-4">
        <SectionHeading title={t("registeredAgentScopes")} titleAs="h2" className="border-b-0 pb-3" />
        {agents.length ? (
          <dl className="grid gap-3 text-sm">
            {agents.map((agent) => {
              const agentId = getAgentId(agent)
              const instanceIds = new Set(agent.instance_ids)
              const scopedServices = services.filter((service) => instanceIds.has(service.instance_id))
              return (
                <div key={agentId || JSON.stringify(agent)} className="grid gap-1">
                  <dt className="font-mono text-muted-foreground">{agentId || "-"}</dt>
                  <dd className="break-words font-mono">
                    {scopedServices.length
                      ? scopedServices.map((service) => service.service_name).join(" · ")
                      : t("noServices")}
                  </dd>
                </div>
              )
            })}
          </dl>
        ) : (
          <PageEmpty title={t("noScopes")} description={t("noScopesDescription")} />
        )}
      </section>
    </div>
  )
}

function AgentPreviewHeader({
  busy,
  canAddService,
  loading,
  onAddService,
  onDetails,
  onHub,
  onRefresh,
  scopeTitle,
}: {
  busy: boolean
  canAddService: boolean
  loading: boolean
  onAddService: () => void
  onDetails: () => void
  onHub: () => void
  onRefresh: () => void
  scopeTitle: string
}) {
  const { t } = useI18n()

  return (
    <div className="flex flex-nowrap items-center justify-between gap-3 border-b pb-2">
      <strong className="min-w-0 flex-1 truncate font-mono text-sm font-medium" title={scopeTitle}>
        {scopeTitle}
      </strong>
      <div className="flex shrink-0 gap-2">
        {canAddService ? (
          <Button size="sm" variant="outline" onClick={onAddService} disabled={busy}>
            <PlusIcon data-icon="inline-start" />
            {t("assignService")}
          </Button>
        ) : null}
        <Button size="sm" variant="outline" onClick={onHub} disabled={loading}>
          HUB
        </Button>
        <Button size="sm" variant="outline" onClick={onDetails} disabled={loading}>
          <EyeIcon data-icon="inline-start" />
          详情
        </Button>
        <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
          <RefreshCwIcon data-icon="inline-start" />
          {t("refresh")}
        </Button>
      </div>
    </div>
  )
}

function ScopeServiceSheet({
  busy,
  onOpenChange,
  onOpenService,
  onRemoveScope,
  open,
  service,
  toolCount,
}: {
  busy: boolean
  onOpenChange: (open: boolean) => void
  onOpenService: (service: ServiceInstance) => void
  onRemoveScope?: (serviceName: string) => void
  open: boolean
  service: ServiceInstance | null
  toolCount: number
}) {
  const { t } = useI18n()

  if (!service) return null

  const endpoint = service.url || service.command || "-"
  const scope = service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`
  const launchLine = getServiceEndpointLabel(service)

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full max-w-md overflow-y-auto sm:max-w-md">
        <SheetHeader>
          <SheetTitle className="font-mono">{service.service_name}</SheetTitle>
          <SheetDescription className="font-mono">{launchLine}</SheetDescription>
        </SheetHeader>

        <dl className="grid gap-4 px-4 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("scope")}</dt>
            <dd className="font-mono">{scope}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("status")}</dt>
            <dd>
              <ServiceStatusBadge state={service.state} />
            </dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("tools")}</dt>
            <dd>{t("toolsAvailable", { count: toolCount })}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("endpoint")}</dt>
            <dd className="break-all font-mono">{String(endpoint)}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("transport")}</dt>
            <dd className="font-mono">{service.transport}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">Instance ID</dt>
            <dd className="break-all font-mono">{service.instance_id}</dd>
          </div>
        </dl>

        <div className="mt-auto flex flex-col gap-2 border-t p-4">
          <Button onClick={() => onOpenService(service)}>
            <LinkIcon data-icon="inline-start" />
            {t("view")} {t("service")}
          </Button>
          {onRemoveScope ? (
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => onRemoveScope(service.service_name)}
            >
              <UnlinkIcon data-icon="inline-start" />
              {t("delete")} {t("scope")}
            </Button>
          ) : null}
        </div>
      </SheetContent>
    </Sheet>
  )
}
