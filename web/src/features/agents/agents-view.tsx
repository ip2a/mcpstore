import { useEffect, useMemo, useState } from "react"
import { EyeIcon, LinkIcon, PlusIcon, RefreshCwIcon, UnlinkIcon } from "lucide-react"

import { EntityRow } from "@/components/shared/entity-row"
import { JsonBlock } from "@/components/shared/json-block"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { useAgentConfigQuery, useStoreConfigQuery } from "@/features/config/queries"
import { useScopesQuery } from "@/features/agents/queries"
import { ServiceRowMeta } from "@/components/shared/service-row-meta"
import { ServiceConnectionMark, ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { AddScopeServiceDialog } from "@/features/agents/add-scope-service-dialog"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { type AgentItem, type ScopeSummary, type ScopeView, type ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { getServiceEndpointLabel } from "@/lib/service-info"
import { cn } from "@/lib/utils"

type ScopeCardId = "store" | string

const ALL_SCOPE_ID = "all"
const STORE_SCOPE_ID = "store"

function scopeIdFromSummary(summary: ScopeSummary): ScopeCardId {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "root") return t("global")
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
  const [activeTab, setActiveTab] = useState<ScopeCardId>("root")
  const [selectedScopeId, setSelectedScopeId] = useState<ScopeCardId>("root")
  const [addScopeDialogOpen, setAddScopeDialogOpen] = useState(false)
  const [previewServiceId, setPreviewServiceId] = useState<string | null>(null)
  const [configScopeId, setConfigScopeId] = useState(ALL_SCOPE_ID)
  const [configDialogOpen, setConfigDialogOpen] = useState(false)
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

  const loadingScope = loadingScopeServices || loadingScopeTools
  const scopeError = scopeServicesError || scopeToolsError
  const scopeErrorMessage = scopeServicesError ? scopeServicesErrorMessage : scopeToolsErrorMessage

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
  }, [activeTab, selectedScopeId])

  function loadScopeConfig() {
    void (configAgentId ? agentConfigQuery.refetch() : storeConfigQuery.refetch())
  }

  function selectScopeTab(scopeId: ScopeCardId) {
    setConfigScopeId(
      scopeId === "root"
        ? ALL_SCOPE_ID
        : scopeId === STORE_SCOPE_ID
          ? scopeId
          : `agent:${scopeId}`,
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
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="text-lg font-semibold">{t("agentWorkspaceTitle")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("agentWorkspaceDescription", { count: props.agents.length })}
        </p>
      </div>

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <Tabs
        value={activeTab}
        onValueChange={(value) => selectScopeTab(value)}
        className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
      >
        <div className="flex min-w-0 shrink-0 items-center gap-3 border-b pb-2">
          <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
            {scopes.map((summary) => {
              const scopeId = scopeIdFromSummary(summary)
              const title = scopeTitleFromSummary(summary, t)
              return (
                <TabsTrigger key={scopeId} value={scopeId} title={title} className="flex-none">
                  {title}
                </TabsTrigger>
              )
            })}
          </TabsList>
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
                  onRemoveScope={props.onRemoveScope}
                  previewService={previewService}
                  previewServiceId={previewServiceId}
                  previewToolCount={previewToolCount}
                  scopeError={scopeError}
                  scopeErrorMessage={scopeErrorMessage}
                  scopeServiceName={scopeServiceName}
                  scopeServices={scopeServices}
                  scopeToolCountByInstance={scopeToolCountByInstance}
                  selectedScope={selectedScope}
                  setAddScopeDialogOpen={setAddScopeDialogOpen}
                  setConfigDialogOpen={setConfigDialogOpen}
                  setPreviewServiceId={setPreviewServiceId}
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
        <RefreshCwIcon data-icon="inline-start" />
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
  onRemoveScope,
  previewService,
  previewServiceId,
  previewToolCount,
  scopeError,
  scopeErrorMessage,
  scopeServiceName,
  scopeServices,
  scopeToolCountByInstance,
  selectedScope,
  setAddScopeDialogOpen,
  setConfigDialogOpen,
  setPreviewServiceId,
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
  onRemoveScope: (agentId: string, serviceName: string) => void
  previewService: ServiceInstance | null
  previewServiceId: string | null
  previewToolCount: number
  scopeError: unknown
  scopeErrorMessage: string
  scopeServiceName: string
  scopeServices: ServiceInstance[]
  scopeToolCountByInstance: Map<string, number>
  selectedScope: ScopeView
  setAddScopeDialogOpen: (open: boolean) => void
  setConfigDialogOpen: (open: boolean) => void
  setPreviewServiceId: (id: string | null) => void
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
                      interactive
                      selected={selected}
                      className="cursor-pointer"
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

      <ScopeServiceSheet
        busy={Boolean(busy)}
        onOpenChange={(open) => {
          if (!open) setPreviewServiceId(null)
        }}
        onOpenService={(service) => {
          setPreviewServiceId(null)
          onOpenService(service)
        }}
        onRemoveScope={
          selectedScope.type === "agent" && activeAgentId
            ? (serviceName) => {
                setPreviewServiceId(null)
                onRemoveScope(activeAgentId, serviceName)
              }
            : undefined
        }
        open={Boolean(previewService)}
        service={previewService}
        toolCount={previewToolCount}
      />
    </>
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
