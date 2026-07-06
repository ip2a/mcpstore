import { useCallback, useEffect, useMemo, useState } from "react"
import {
  ActivityIcon,
  ArrowLeftIcon,
  DatabaseIcon,
  LinkIcon,
  PlusIcon,
  RefreshCwIcon,
  SettingsIcon,
  Trash2Icon,
  UnlinkIcon,
} from "lucide-react"
import { toast } from "sonner"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { CacheView } from "@/features/cache/cache-view"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { ConfigView, type ResetTarget } from "@/features/config/config-view"
import { AddServiceView } from "@/features/services/add-service-view"
import { ServiceTable } from "@/features/services/service-table"
import { SettingsDialog } from "@/features/settings/settings-dialog"
import { RunToolDialog, ToolDetailDialog, type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { DetailHeader } from "@/components/shared/detail-header"
import { EntityRow } from "@/components/shared/entity-row"
import { JsonBlock } from "@/components/shared/json-block"
import { MetaLine } from "@/components/shared/meta-line"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { ToolCard } from "@/components/shared/tool-card"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Toaster } from "@/components/ui/sonner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { TooltipProvider } from "@/components/ui/tooltip"
import { HomeHero } from "@/components/home-hero"
import { useDashboard } from "@/hooks/use-dashboard"
import { formatDateTime } from "@/lib/format"
import { getToolServiceName, toolKey } from "@/lib/tool-info"
import { useUiStore } from "@/stores/ui-store"
import {
  assignService,
  callAgentTool,
  callStoreTool,
  callTool,
  checkServices,
  connectService,
  disconnectService,
  listAgentServices,
  listAgentTools,
  listTools,
  removeService,
  resetAgentConfig,
  resetConfig,
  restartService,
  serviceInfo,
  serviceStatus,
  unassignService,
  type AgentItem,
  type CacheBackend,
  type ServiceEntry,
  type ToolInfo,
} from "@/lib/api"

type View =
  | { name: "services" }
  | { name: "agents" }
  | { name: "tools" }
  | { name: "config" }
  | { name: "cache" }
  | { name: "add" }
  | { name: "service"; serviceName: string }

const navItems: Array<{ view: View["name"]; label: string }> = [
  { view: "services", label: "服务" },
  { view: "agents", label: "Agent" },
  { view: "tools", label: "工具" },
  { view: "config", label: "配置" },
  { view: "cache", label: "缓存" },
]

function viewTitle(view: View): string {
  if (view.name === "service") return view.serviceName
  if (view.name === "add") return "添加服务"
  return navItems.find((item) => item.view === view.name)?.label || "服务"
}

export function App() {
  const { services, agents, agentMap, backend, loading, error: dashboardError, refresh } = useDashboard()
  const [view, setView] = useState<View>({ name: "services" })
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)
  const [cacheDialog, setCacheDialog] = useState(false)
  const settingsDialogOpen = useUiStore((state) => state.settingsDialogOpen)
  const setSettingsDialogOpen = useUiStore((state) => state.setSettingsDialogOpen)
  const [deleteTarget, setDeleteTarget] = useState<ServiceEntry | null>(null)
  const [resetTarget, setResetTarget] = useState<ResetTarget | null>(null)
  const [cacheRevision, setCacheRevision] = useState(0)
  const [busy, setBusy] = useState<string | null>(null)

  const selectedService = view.name === "service" ? services.find((service) => service.name === view.serviceName) : undefined
  const isHome = view.name === "services"
  const pageTitle = viewTitle(view)

  async function runAction(label: string, action: () => Promise<unknown>) {
    setBusy(label)
    try {
      await action()
      toast.success("操作已完成")
      await refresh()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败")
    } finally {
      setBusy(null)
    }
  }

  async function confirmReset(target: ResetTarget) {
    if (target.scope === "store") {
      await runAction("reset:store", resetConfig)
    } else {
      await runAction(`reset:${target.agentId}`, () => resetAgentConfig(target.agentId))
    }
  }

  function openServiceToolRunner(service: ServiceEntry, tool: ToolInfo) {
    setToolDialog({
      tool,
      sourceLabel: service.name,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  function openServiceToolDetail(service: ServiceEntry, tool: ToolInfo) {
    setToolDetail({
      tool,
      sourceLabel: service.name,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  return (
    <TooltipProvider>
      <div className="min-h-dvh bg-background">
        <div className="mx-auto grid h-dvh w-[min(1280px,calc(100vw-24px))] grid-rows-[auto_minmax(0,1fr)] overflow-hidden pb-4">
          <header className="mb-3 flex min-h-16 items-center justify-between gap-4 border-b py-3">
            <div className="flex min-w-0 items-center gap-3">
              <button className="font-mono font-bold" type="button" onClick={() => setView({ name: "services" })}>
                mcpstore
              </button>
              <div className="h-5 w-px bg-border" />
              <span className="truncate text-sm font-semibold">{pageTitle}</span>
            </div>

            <nav className="flex flex-wrap items-center justify-end gap-2">
              {!isHome ? (
                <Button type="button" variant="outline" size="sm" onClick={() => setView({ name: "services" })}>
                  <ArrowLeftIcon data-icon="inline-start" />
                  返回
                </Button>
              ) : null}
              {navItems.map((item) =>
                view.name === item.view ? null : (
                  <Button key={item.view} variant="outline" size="sm" onClick={() => setView({ name: item.view } as View)}>
                    {item.label}
                  </Button>
                ),
              )}
              <Button variant="outline" size="sm" onClick={() => setView({ name: "add" })}>
                <PlusIcon data-icon="inline-start" />
                添加
              </Button>
              <Button variant="outline" size="sm" onClick={() => setSettingsDialogOpen(true)}>
                <SettingsIcon data-icon="inline-start" />
                设置
              </Button>
            </nav>
          </header>

          <main className="flex min-h-0 flex-col gap-6 overflow-auto py-3">
          {view.name === "add" ? (
            <AddServiceView agents={agents} onBack={() => setView({ name: "services" })} onAdded={refresh} />
          ) : selectedService ? (
            <ServiceDetailView
              service={selectedService}
              busy={busy}
              onBack={() => setView({ name: "services" })}
              onRunTool={(tool) => openServiceToolRunner(selectedService, tool)}
              onToolDetail={(tool) => openServiceToolDetail(selectedService, tool)}
              onConnect={() => runAction(`connect:${selectedService.name}`, () => connectService(selectedService.name))}
              onDisconnect={() => runAction(`disconnect:${selectedService.name}`, () => disconnectService(selectedService.name))}
              onRestart={() => runAction(`restart:${selectedService.name}`, () => restartService(selectedService.name))}
              onDelete={() => setDeleteTarget(selectedService)}
            />
          ) : view.name === "agents" ? (
            <AgentsView
              agents={agents}
              services={services}
              loading={loading}
              busy={busy}
              onRefresh={refresh}
              onAssign={(agentId, serviceName) => runAction(`assign:${agentId}:${serviceName}`, () => assignService(agentId, serviceName))}
              onOpenService={(serviceName) => setView({ name: "service", serviceName })}
              onUnassign={(agentId, serviceName) => runAction(`unassign:${agentId}:${serviceName}`, () => unassignService(agentId, serviceName))}
            />
          ) : view.name === "tools" ? (
            <ToolsView
              agents={agents}
              services={services}
              onToolDetail={setToolDetail}
              onRunTool={setToolDialog}
            />
          ) : view.name === "config" ? (
            <ConfigView agents={agents} resetTarget={resetTarget} onResetTarget={setResetTarget} />
          ) : view.name === "cache" ? (
            <CacheView backend={backend} revision={cacheRevision} onRefreshDashboard={refresh} onSwitch={() => setCacheDialog(true)} />
          ) : (
            <ServicesView
              services={services}
              agents={agents}
              agentMap={agentMap}
              backend={backend}
              busy={busy}
              error={dashboardError}
              loading={loading}
              onCache={() => setView({ name: "cache" })}
              onCheck={() => runAction("check:services", checkServices)}
              onConnect={(service) => runAction(`connect:${service.name}`, () => connectService(service.name))}
              onDelete={setDeleteTarget}
              onDisconnect={(service) => runAction(`disconnect:${service.name}`, () => disconnectService(service.name))}
              onOpen={(service) => setView({ name: "service", serviceName: service.name })}
              onRefresh={refresh}
              onRestart={(service) => runAction(`restart:${service.name}`, () => restartService(service.name))}
            />
          )}
          </main>
        </div>
      </div>

      <RunToolDialog state={toolDialog} onOpenChange={(open) => !open && setToolDialog(null)} />
      <ToolDetailDialog
        state={toolDetail}
        onOpenChange={(open) => !open && setToolDetail(null)}
        onRun={(state) => {
          if (!state.onRun) return
          setToolDialog({ tool: state.tool, sourceLabel: state.sourceLabel, onRun: state.onRun })
        }}
      />
      <SwitchCacheDialog
        open={cacheDialog}
        current={backend}
        onOpenChange={setCacheDialog}
        onChanged={async () => {
          await refresh()
          setCacheRevision((value) => value + 1)
        }}
      />
      <SettingsDialog open={settingsDialogOpen} onOpenChange={setSettingsDialogOpen} />
      <DeleteServiceDialog
        service={deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        onConfirm={(service) => runAction(`delete:${service.name}`, () => removeService(service.name)).then(() => setView({ name: "services" }))}
      />
      <ResetConfigDialog
        target={resetTarget}
        onOpenChange={(open) => !open && setResetTarget(null)}
        onConfirm={(target) => confirmReset(target).then(() => setResetTarget(null))}
      />
      <Toaster />
    </TooltipProvider>
  )
}

function ServicesView(props: {
  services: ServiceEntry[]
  agents: AgentItem[]
  agentMap: Map<string, string>
  backend?: CacheBackend
  busy: string | null
  error: string | null
  loading: boolean
  onCache: () => void
  onCheck: () => void
  onConnect: (service: ServiceEntry) => void
  onDelete: (service: ServiceEntry) => void
  onDisconnect: (service: ServiceEntry) => void
  onOpen: (service: ServiceEntry) => void
  onRefresh: () => void
  onRestart: (service: ServiceEntry) => void
}) {
  const [agentFilter, setAgentFilter] = useState("store")
  const [query, setQuery] = useState("")
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const filteredServices = useMemo(() => {
    return props.services.filter((service) => {
      const inAgent = agentFilter === "store" || props.agentMap.get(service.name) === agentFilter
      const text = `${service.name} ${service.transport || ""} ${service.config?.description || ""}`.toLowerCase()
      return inAgent && text.includes(query.trim().toLowerCase())
    })
  }, [agentFilter, props.agentMap, props.services, query])
  const totals = useMemo(() => {
    const count = (status: string) => filteredServices.filter((service) => service.status === status).length
    return {
      services: filteredServices.length,
      tools: filteredServices.reduce((sum, service) => sum + (service.tools?.length || 0), 0),
      connected: count("Connected"),
      disconnected: count("Disconnected"),
      connecting: count("Connecting"),
      error: count("Error"),
    }
  }, [filteredServices])

  return (
    <>
      <HomeHero
        backend={props.backend}
        stats={{
          loading: props.loading,
          services: totals.services,
          connected: totals.connected,
          disconnected: totals.disconnected,
          connecting: totals.connecting,
          error: totals.error,
          tools: totals.tools,
          agents: agentIds.length,
        }}
      />

      <PanelCard>
        <SectionHeading
          title="MCP 服务列表"
          titleAs="h2"
          description={`${filteredServices.length} services`}
          className="border-b-0 pb-0"
          actions={
            <Button variant="outline" size="sm" onClick={props.onCache}>
              <DatabaseIcon data-icon="inline-start" />
              缓存
            </Button>
          }
        />
        <div className="flex flex-col gap-4">
          <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_220px_auto_auto]">
            <SearchBox placeholder="Search services" value={query} onChange={setQuery} />
            <Select value={agentFilter} onValueChange={setAgentFilter}>
              <SelectTrigger>
                <SelectValue placeholder="Agent" />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="store">Store</SelectItem>
                  {agentIds.map((agentId) => (
                    <SelectItem key={agentId} value={agentId}>
                      {agentId}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
            <Button variant="outline" onClick={props.onRefresh} disabled={props.loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            <Button variant="outline" onClick={props.onCheck} disabled={Boolean(props.busy)}>
              <ActivityIcon data-icon="inline-start" />
              检查
            </Button>
          </div>
          {props.error ? (
            <PageError title="Dashboard failed to load" message={props.error} onRefresh={props.onRefresh} />
          ) : props.loading ? (
            <PageSkeleton />
          ) : filteredServices.length ? (
            <ServiceTable {...props} services={filteredServices} />
          ) : (
            <PageEmpty title="No services" description="No MCP services are available in the current view." onRefresh={props.onRefresh} />
          )}
        </div>
      </PanelCard>
    </>
  )
}

function AgentsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  loading: boolean
  busy: string | null
  onAssign: (agentId: string, serviceName: string) => void
  onOpenService: (serviceName: string) => void
  onRefresh: () => void
  onUnassign: (agentId: string, serviceName: string) => void
}) {
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const selectedAgentId = useUiStore((state) => state.selectedAgentId)
  const setSelectedAgentId = useUiStore((state) => state.setSelectedAgentId)
  const [typedAgentId, setTypedAgentId] = useState("")
  const [assignTarget, setAssignTarget] = useState(props.services[0]?.name || "")
  const [agentServices, setAgentServices] = useState<ServiceEntry[]>([])
  const [agentTools, setAgentTools] = useState<ToolInfo[]>([])
  const [agentServicesError, setAgentServicesError] = useState<string | null>(null)
  const [agentToolsError, setAgentToolsError] = useState<string | null>(null)
  const [loadingAgent, setLoadingAgent] = useState(false)
  const activeAgentId = (typedAgentId.trim() || selectedAgentId || "").trim()

  useEffect(() => {
    if (!selectedAgentId && agentIds[0]) setSelectedAgentId(agentIds[0])
  }, [agentIds, selectedAgentId])

  useEffect(() => {
    if (!assignTarget && props.services[0]?.name) setAssignTarget(props.services[0].name)
  }, [assignTarget, props.services])

  const loadAgentScope = useCallback(
    async (agentId: string, options: { cancelled?: () => boolean } = {}) => {
      if (!agentId) {
        setAgentServices([])
        setAgentTools([])
        setAgentServicesError(null)
        setAgentToolsError(null)
        return
      }
      setLoadingAgent(true)
      setAgentServicesError(null)
      setAgentToolsError(null)
      try {
        const [servicesResult, toolsResult] = await Promise.allSettled([listAgentServices(agentId), listAgentTools(agentId)])
        if (options.cancelled?.()) return
        if (servicesResult.status === "fulfilled") {
          setAgentServices(servicesResult.value)
        } else {
          const message = servicesResult.reason instanceof Error ? servicesResult.reason.message : "Agent services 加载失败"
          setAgentServices([])
          setAgentServicesError(message)
          toast.error(message)
        }
        if (toolsResult.status === "fulfilled") {
          setAgentTools(toolsResult.value)
        } else {
          const message = toolsResult.reason instanceof Error ? toolsResult.reason.message : "Agent tools 加载失败"
          setAgentTools([])
          setAgentToolsError(message)
          toast.error(message)
        }
      } finally {
        if (!options.cancelled?.()) setLoadingAgent(false)
      }
    },
    [],
  )

  useEffect(() => {
    let cancelled = false
    void loadAgentScope(activeAgentId, { cancelled: () => cancelled })
    return () => {
      cancelled = true
    }
  }, [activeAgentId, loadAgentScope, props.busy])

  return (
    <>
      <DetailHeader
        eyebrow="Agent 管理"
        title="Agent Workspace"
        actions={
          <Button variant="outline" onClick={props.onRefresh} disabled={props.loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <TwoPanePage variant="page">
        <div className="flex min-w-0 flex-col gap-4">
          <PanelCard>
            <SectionHeading title="Agent List" titleAs="h2" description={`${props.agents.length} items`} className="border-b-0 pb-0" />
            <div>
              {props.loading ? (
                <PageSkeleton />
              ) : props.agents.length ? (
                <div className="flex flex-col gap-2">
                  {props.agents.map((agent) => {
                    const agentId = getAgentId(agent)
                    const serviceNames = getAgentServices(agent)
                    return (
                      <SelectableRowButton
                        key={agentId || JSON.stringify(agent)}
                        disabled={!agentId}
                        meta={`${serviceNames.length} services · ${agentId === activeAgentId ? agentTools.length : "-"} tools`}
                        onClick={() => setSelectedAgentId(agentId || null)}
                        selected={agentId === activeAgentId}
                        title={agentId || "-"}
                        trailing={agentId === activeAgentId ? <Badge variant="outline">active</Badge> : null}
                      />
                    )
                  })}
                </div>
              ) : (
                <PageEmpty title="No agents" description="Agent records will appear after services are assigned." onRefresh={props.onRefresh} />
              )}
            </div>
          </PanelCard>

          <PanelCard>
            <SectionHeading title="Agent Scope" titleAs="h2" description={activeAgentId || "No agent selected"} className="border-b-0 pb-0" />
            <div>
              <FieldGroup>
                <Field>
                  <FieldLabel>Known Agent</FieldLabel>
                  <Select value={selectedAgentId || "none"} onValueChange={(value) => setSelectedAgentId(value === "none" ? null : value)}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="none">None</SelectItem>
                        {agentIds.map((agentId) => (
                          <SelectItem key={agentId} value={agentId}>
                            {agentId}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel htmlFor="agent-id">Agent ID</FieldLabel>
                  <Input id="agent-id" value={typedAgentId} onChange={(event) => setTypedAgentId(event.target.value)} placeholder="agent-a" />
                </Field>
                <Field>
                  <FieldLabel>Assign Service</FieldLabel>
                  <Select value={assignTarget || "none"} onValueChange={(value) => setAssignTarget(value === "none" ? "" : value)}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="none">None</SelectItem>
                        {props.services.map((service) => (
                          <SelectItem key={service.name} value={service.name}>
                            {service.name}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Button disabled={!activeAgentId || !assignTarget || Boolean(props.busy)} onClick={() => props.onAssign(activeAgentId, assignTarget)}>
                  Assign
                </Button>
              </FieldGroup>
            </div>
          </PanelCard>
        </div>

        <div className="flex min-w-0 flex-col gap-4">
          <PanelCard>
            <SectionHeading title="Agent Services" titleAs="h2" description={loadingAgent ? "Loading" : `${agentServices.length} items`} className="border-b-0 pb-0" />
            <div>
              {agentServicesError ? (
                <PageError title="Agent services failed to load" message={agentServicesError} onRefresh={() => loadAgentScope(activeAgentId)} />
              ) : loadingAgent ? (
                <PageSkeleton />
              ) : agentServices.length ? (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Service</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {agentServices.map((service) => (
                      <TableRow key={service.name}>
                        <TableCell className="font-medium">{service.name}</TableCell>
                        <TableCell>
                          <ServiceStatusBadge status={service.status} />
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-2">
                            <Button size="sm" variant="outline" onClick={() => props.onOpenService(service.name)}>
                              View
                            </Button>
                            <Button size="sm" variant="outline" onClick={() => props.onUnassign(activeAgentId, service.name)} disabled={!activeAgentId || Boolean(props.busy)}>
                              Unassign
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              ) : (
                <PageEmpty title="No services" description="No MCP services are available for this agent." onRefresh={() => loadAgentScope(activeAgentId)} />
              )}
            </div>
          </PanelCard>

          <PanelCard>
            <SectionHeading title="Agent Tools" titleAs="h2" description={loadingAgent ? "Loading" : `${agentTools.length} items`} className="border-b-0 pb-0" />
            <div>
              {agentToolsError ? (
                <PageError title="Agent tools failed to load" message={agentToolsError} onRefresh={() => loadAgentScope(activeAgentId)} />
              ) : loadingAgent ? (
                <PageSkeleton />
              ) : agentTools.length ? (
                <div className="flex flex-col gap-3">
                  {agentTools.slice(0, 8).map((tool) => (
                    <EntityRow key={toolKey(tool)} actions={<Badge variant="outline">tool</Badge>}>
                      <div className="flex min-w-0 flex-col gap-1">
                        <p className="truncate text-sm font-medium">{tool.name}</p>
                        <p className="truncate text-sm text-muted-foreground">{tool.description || "No description"}</p>
                      </div>
                    </EntityRow>
                  ))}
                </div>
              ) : (
                <PageEmpty title="No tools" description="No tools are available for this agent." onRefresh={() => loadAgentScope(activeAgentId)} />
              )}
            </div>
          </PanelCard>
        </div>
      </TwoPanePage>
    </>
  )
}

function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  onRunTool: (state: ToolDialogState) => void
  onToolDetail: (state: ToolDetailState) => void
}) {
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [serviceName, setServiceName] = useState("all")
  const [query, setQuery] = useState("")
  const [tools, setTools] = useState<ToolInfo[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  async function loadTools() {
    setLoading(true)
    try {
      setError(null)
      const nextTools = scope === "agent" && agentId ? await listAgentTools(agentId, serviceName === "all" ? undefined : serviceName) : await listTools(serviceName === "all" ? undefined : serviceName)
      setTools(nextTools)
    } catch (err) {
      const message = err instanceof Error ? err.message : "工具加载失败"
      setError(message)
      toast.error(message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadTools()
  }, [scope, agentId, serviceName])

  const visibleTools = tools.filter((tool) => {
    const text = `${tool.name} ${tool.description || ""} ${getToolServiceName(tool) || ""}`.toLowerCase()
    return text.includes(query.trim().toLowerCase())
  })

  function makeRunner(tool: ToolInfo): NonNullable<ToolDialogState> {
    const sourceLabel = scope === "agent" ? `Agent ${agentId}` : getToolServiceName(tool) || serviceName
    return {
      tool,
      sourceLabel,
      onRun: (args) => (scope === "agent" ? callAgentTool(agentId, tool.name, args) : callStoreTool(tool.name, args)),
    }
  }

  return (
    <>
      <DetailHeader
        eyebrow="工具管理"
        title="Tool Registry"
        actions={
          <Button variant="outline" onClick={loadTools} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <PanelCard>
        <SectionHeading title="Filters" titleAs="h2" description={`${visibleTools.length} tools`} className="border-b-0 pb-0" />
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_180px_220px_220px]">
          <SearchBox placeholder="Search tools" value={query} onChange={setQuery} />
          <Select value={scope} onValueChange={setScope}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="store">Store</SelectItem>
                <SelectItem value="agent">Agent</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
          <Select value={agentId || "none"} onValueChange={(value) => setAgentId(value === "none" ? "" : value)} disabled={scope !== "agent"}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="none">No agent</SelectItem>
                {agentIds.map((id) => (
                  <SelectItem key={id} value={id}>
                    {id}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <Select value={serviceName} onValueChange={setServiceName}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="all">All services</SelectItem>
                {props.services.map((service) => (
                  <SelectItem key={service.name} value={service.name}>
                    {service.name}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
      </PanelCard>

      {error ? (
        <PageError title="Tools failed to load" message={error} onRefresh={loadTools} />
      ) : loading ? (
        <PageSkeleton />
      ) : visibleTools.length ? (
        <section className="grid gap-4 lg:grid-cols-2">
          {visibleTools.map((tool) => {
            const runner = makeRunner(tool)
            return (
              <ToolCard
                key={toolKey(tool)}
                tool={tool}
                sourceLabel={runner.sourceLabel}
                onRun={() => props.onRunTool(runner)}
                onDetail={() => props.onToolDetail(runner)}
              />
            )
          })}
        </section>
      ) : (
        <PageEmpty title="No tools" description="No tools are available in the current scope." onRefresh={loadTools} />
      )}
    </>
  )
}

function ServiceDetailView(props: {
  service: ServiceEntry
  busy: string | null
  onBack: () => void
  onRunTool: (tool: ToolInfo) => void
  onToolDetail: (tool: ToolInfo) => void
  onConnect: () => void
  onDisconnect: () => void
  onRestart: () => void
  onDelete: () => void
}) {
  const [detail, setDetail] = useState<ServiceEntry | null>(null)
  const [statusReport, setStatusReport] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const service = detail || props.service
  const endpoint = service.url || service.command || "-"
  const tools = service.tools || []

  async function loadDetail() {
    setLoading(true)
    try {
      setError(null)
      const [nextDetail, nextStatus] = await Promise.all([serviceInfo(props.service.name), serviceStatus(props.service.name).catch(() => null)])
      setDetail(nextDetail)
      setStatusReport(nextStatus)
    } catch (err) {
      const message = err instanceof Error ? err.message : "服务详情加载失败"
      setError(message)
      toast.error(message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadDetail()
  }, [props.service.name])

  return (
    <>
      <DetailHeader
        badges={<ServiceStatusBadge status={service.status} />}
        eyebrow="服务详情"
        meta={
          <div className="flex flex-wrap gap-2 text-sm text-muted-foreground">
            <span>tools · {tools.length}</span>
            <span>added · {formatDateTime(service.added_time)}</span>
          </div>
        }
        title={service.name}
        actions={
          <>
            <Button variant="outline" onClick={props.onBack}>
              <ArrowLeftIcon data-icon="inline-start" />
              Back
            </Button>
            <Button variant="outline" onClick={loadDetail} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            {service.status === "Connected" ? (
              <Button variant="outline" onClick={props.onDisconnect} disabled={Boolean(props.busy)}>
                <UnlinkIcon data-icon="inline-start" />
                Disconnect
              </Button>
            ) : (
              <Button onClick={props.onConnect} disabled={Boolean(props.busy)}>
                <LinkIcon data-icon="inline-start" />
                Connect
              </Button>
            )}
            <Button variant="outline" onClick={props.onRestart} disabled={Boolean(props.busy)}>
              <RefreshCwIcon data-icon="inline-start" />
              Restart
            </Button>
            <Button variant="destructive" onClick={props.onDelete} disabled={Boolean(props.busy)}>
              <Trash2Icon data-icon="inline-start" />
              Delete
            </Button>
          </>
        }
      />

      <MetricGrid columns="four">
        <MetricTile variant="compact" label="Name" value={service.name} title={service.name} />
        <MetricTile variant="compact" label="Endpoint" value={String(endpoint)} title={String(endpoint)} />
        <MetricTile variant="compact" label="Agent" value={String(service.agent_id || "store")} />
        <MetricTile variant="compact" label="Tools" value={String(tools.length)} />
      </MetricGrid>

      <PanelCard>
        <SectionHeading title="Service Info" titleAs="h2" className="border-b-0 pb-0" actions={<ServiceStatusBadge status={service.status} />} />
        <div className="grid gap-4 text-sm md:grid-cols-2">
          <MetaLine label="Transport" value={String(service.transport || "unknown")} valueClassName="font-mono" />
          <MetaLine label="Original" value={String(service.original_name || service.name)} valueClassName="font-mono" />
          <MetaLine label="Added" value={formatDateTime(service.added_time)} valueClassName="font-mono" />
          <MetaLine label="Command" value={String(service.command || "-")} valueClassName="font-mono" />
          <MetaLine label="URL" value={String(service.url || "-")} valueClassName="font-mono" />
        </div>
      </PanelCard>

      <section className="flex flex-col gap-3">
        <SectionHeading title="Tool List" titleAs="h2" description={`${tools.length} items`} className="border-b-0 pb-0" />
        {tools.length ? (
          <div className="grid gap-4 lg:grid-cols-2">
            {tools.map((tool) => (
              <ToolCard
                key={toolKey(tool)}
                tool={tool}
                sourceLabel={service.name}
                onRun={() => props.onRunTool(tool)}
                onDetail={() => props.onToolDetail(tool)}
              />
            ))}
          </div>
        ) : (
          <PageEmpty title="No tools found" description="Tool definitions will appear after the service is connected." onRefresh={loadDetail} />
        )}
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <PanelCard>
          <SectionHeading title="Status" titleAs="h2" className="border-b-0 pb-0" />
          {error ? <PageError title="Service status failed to load" message={error} onRefresh={loadDetail} /> : <JsonBlock value={statusReport || { status: service.status || "Unknown" }} />}
        </PanelCard>
        <PanelCard>
          <SectionHeading title="Raw Detail" titleAs="h2" className="border-b-0 pb-0" />
          <JsonBlock value={service} />
        </PanelCard>
      </section>
    </>
  )
}

function DeleteServiceDialog({ service, onOpenChange, onConfirm }: { service: ServiceEntry | null; onOpenChange: (open: boolean) => void; onConfirm: (service: ServiceEntry) => void }) {
  return (
    <AlertDialog open={Boolean(service)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <Trash2Icon />
          </AlertDialogMedia>
          <AlertDialogTitle>Delete service?</AlertDialogTitle>
          <AlertDialogDescription>{service ? `This removes ${service.name} from mcpstore.` : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => service && onConfirm(service)}>Delete</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function ResetConfigDialog({ target, onOpenChange, onConfirm }: { target: ResetTarget | null; onOpenChange: (open: boolean) => void; onConfirm: (target: ResetTarget) => void }) {
  const label = target?.scope === "agent" ? `Agent ${target.agentId}` : "Store"
  return (
    <AlertDialog open={Boolean(target)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <RefreshCwIcon />
          </AlertDialogMedia>
          <AlertDialogTitle>Reset config?</AlertDialogTitle>
          <AlertDialogDescription>{target ? `${label} config will be reset.` : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => target && onConfirm(target)}>Reset</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function getAgentId(agent: AgentItem) {
  return String(agent.agent_id || agent.id || "")
}

function getAgentServices(agent: AgentItem) {
  return (agent.services || agent.service_names || []).map(String)
}
