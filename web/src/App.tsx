import { useEffect, useMemo, useState, type FormEvent } from "react"
import {
  ActivityIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  ClipboardIcon,
  DatabaseIcon,
  EyeIcon,
  LinkIcon,
  MoreHorizontalIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  Trash2Icon,
  UnlinkIcon,
  WrenchIcon,
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { SettingsDialog } from "@/features/settings/settings-dialog"
import { DetailHeader } from "@/components/shared/detail-header"
import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { EntityRow } from "@/components/shared/entity-row"
import { MetaLine } from "@/components/shared/meta-line"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Toaster } from "@/components/ui/sonner"
import { Spinner } from "@/components/ui/spinner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { TooltipProvider } from "@/components/ui/tooltip"
import { HomeHero } from "@/components/home-hero"
import { useDashboard } from "@/hooks/use-dashboard"
import { formatDateTime } from "@/lib/format"
import { useUiStore } from "@/stores/ui-store"
import {
  addService,
  assignService,
  cacheHealth,
  cacheInspect,
  callAgentTool,
  callStoreTool,
  callTool,
  checkServices,
  connectService,
  disconnectService,
  listAgentServices,
  listAgentTools,
  listTools,
  parseKvLines,
  removeService,
  resetAgentConfig,
  resetConfig,
  restartService,
  serviceInfo,
  serviceStatus,
  showAgentConfig,
  showConfig,
  switchCache,
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

type ToolDialogState = {
  tool: ToolInfo
  sourceLabel: string
  onRun: (args: Record<string, unknown>) => Promise<unknown>
} | null

type ToolDetailState = {
  tool: ToolInfo
  sourceLabel: string
  onRun?: (args: Record<string, unknown>) => Promise<unknown>
} | null

type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }

const cacheOptions: CacheBackend[] = ["memory", "redis", "openkeyv_memory", "openkeyv_redis"]
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
  const { services, agents, agentMap, backend, loading, refresh } = useDashboard()
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
          {props.loading ? (
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

  useEffect(() => {
    if (!activeAgentId) {
      setAgentServices([])
      setAgentTools([])
      setAgentServicesError(null)
      setAgentToolsError(null)
      return
    }
    let cancelled = false
    setLoadingAgent(true)
    setAgentServicesError(null)
    setAgentToolsError(null)
    Promise.allSettled([listAgentServices(activeAgentId), listAgentTools(activeAgentId)])
      .then(([servicesResult, toolsResult]) => {
        if (cancelled) return
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
      })
      .finally(() => {
        if (!cancelled) setLoadingAgent(false)
      })
    return () => {
      cancelled = true
    }
  }, [activeAgentId, props.busy])

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
                <PageError title="Agent services failed to load" message={agentServicesError} />
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
                          <StatusBadge status={service.status} />
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
                <PageEmpty title="No services" description="No MCP services are available for this agent." />
              )}
            </div>
          </PanelCard>

          <PanelCard>
            <SectionHeading title="Agent Tools" titleAs="h2" description={loadingAgent ? "Loading" : `${agentTools.length} items`} className="border-b-0 pb-0" />
            <div>
              {agentToolsError ? (
                <PageError title="Agent tools failed to load" message={agentToolsError} />
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
                <PageEmpty title="No tools" description="No tools are available for this agent." />
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
        <PageError title="Tools failed to load" message={error} />
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

function ConfigView(props: { agents: AgentItem[]; resetTarget: ResetTarget | null; onResetTarget: (target: ResetTarget | null) => void }) {
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const [activeTab, setActiveTab] = useState("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [storeConfig, setStoreConfig] = useState<unknown>(null)
  const [agentConfig, setAgentConfig] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  async function loadConfig() {
    setLoading(true)
    try {
      setError(null)
      const store = await showConfig()
      setStoreConfig(store)
      if (agentId) {
        setAgentConfig(await showAgentConfig(agentId))
      } else {
        setAgentConfig(null)
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "配置加载失败"
      setError(message)
      toast.error(message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadConfig()
  }, [agentId, props.resetTarget])

  return (
    <>
      <DetailHeader
        eyebrow="配置管理"
        title="Configuration"
        actions={
          <Button variant="outline" onClick={loadConfig} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="store">Store</TabsTrigger>
          <TabsTrigger value="agent">Agent</TabsTrigger>
        </TabsList>
        <TabsContent value="store">
          <PanelCard>
            <SectionHeading
              title="Store Config"
              titleAs="h2"
              className="border-b-0 pb-0"
              actions={<Button variant="outline" size="sm" onClick={() => props.onResetTarget({ scope: "store" })}>
                <RefreshCwIcon data-icon="inline-start" />
                Reset
              </Button>}
            />
            {error ? (
              <PageError title="Configuration failed to load" message={error} />
            ) : loading && !storeConfig ? (
              <PageSkeleton />
            ) : (
              <JsonBlock value={storeConfig || {}} />
            )}
          </PanelCard>
        </TabsContent>
        <TabsContent value="agent">
          <PanelCard>
            <SectionHeading
              title="Agent Config"
              titleAs="h2"
              description={agentId || "No agent selected"}
              className="border-b-0 pb-0"
              actions={<Button variant="outline" size="sm" disabled={!agentId} onClick={() => props.onResetTarget({ scope: "agent", agentId })}>
                <RefreshCwIcon data-icon="inline-start" />
                Reset
              </Button>}
            />
            <Select value={agentId || "none"} onValueChange={(value) => setAgentId(value === "none" ? "" : value)}>
              <SelectTrigger className="w-full md:w-80">
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
            {error ? (
              <PageError title="Agent config failed to load" message={error} />
            ) : loading && !agentConfig ? (
              <PageSkeleton />
            ) : (
              <JsonBlock value={agentConfig || {}} />
            )}
          </PanelCard>
        </TabsContent>
      </Tabs>
    </>
  )
}

function CacheView(props: { backend?: CacheBackend; revision: number; onRefreshDashboard: () => Promise<void>; onSwitch: () => void }) {
  const [healthReport, setHealthReport] = useState<unknown>(null)
  const [inspectReport, setInspectReport] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function loadCache() {
    setLoading(true)
    try {
      setError(null)
      const [health, inspect] = await Promise.all([cacheHealth(), cacheInspect()])
      setHealthReport(health)
      setInspectReport(inspect)
      await props.onRefreshDashboard()
    } catch (err) {
      const message = err instanceof Error ? err.message : "缓存加载失败"
      setError(message)
      toast.error(message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadCache()
  }, [props.revision])

  return (
    <>
      <DetailHeader
        eyebrow="缓存管理"
        title="Cache Storage"
        actions={
          <>
            <Button variant="outline" onClick={loadCache} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            <Button onClick={props.onSwitch}>
              <DatabaseIcon data-icon="inline-start" />
              切换
            </Button>
          </>
        }
      />

      <MetricGrid columns="three">
        <MetricTile variant="compact" label="Current backend" value={props.backend || "unknown"} />
        <MetricTile variant="compact" label="Health keys" value={countKeys(healthReport)} />
        <MetricTile variant="compact" label="Inspect keys" value={countKeys(inspectReport)} />
      </MetricGrid>

      <section className="grid gap-4 lg:grid-cols-2">
        <PanelCard>
          <SectionHeading title="Health" titleAs="h2" description="/cache/health" className="border-b-0 pb-0" />
          {error ? (
            <PageError title="Cache health failed to load" message={error} />
          ) : loading && !healthReport ? (
            <PageSkeleton />
          ) : (
            <JsonBlock value={healthReport || {}} />
          )}
        </PanelCard>
        <PanelCard>
          <SectionHeading title="Inspect" titleAs="h2" description="/cache/inspect" className="border-b-0 pb-0" />
          {error ? (
            <PageError title="Cache inspect failed to load" message={error} />
          ) : loading && !inspectReport ? (
            <PageSkeleton />
          ) : (
            <JsonBlock value={inspectReport || {}} />
          )}
        </PanelCard>
      </section>
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
        badges={<StatusBadge status={service.status} />}
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

      <section className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <InfoCard label="Name" value={service.name} />
        <InfoCard label="Endpoint" value={String(endpoint)} />
        <InfoCard label="Agent" value={String(service.agent_id || "store")} />
        <InfoCard label="Tools" value={String(tools.length)} />
      </section>

      <PanelCard>
        <SectionHeading title="Service Info" titleAs="h2" className="border-b-0 pb-0" actions={<StatusBadge status={service.status} />} />
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
          <PageEmpty title="No tools found" description="Tool definitions will appear after the service is connected." />
        )}
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <PanelCard>
          <SectionHeading title="Status" titleAs="h2" className="border-b-0 pb-0" />
          {error ? <PageError title="Service status failed to load" message={error} /> : <JsonBlock value={statusReport || { status: service.status || "Unknown" }} />}
        </PanelCard>
        <PanelCard>
          <SectionHeading title="Raw Detail" titleAs="h2" className="border-b-0 pb-0" />
          <JsonBlock value={service} />
        </PanelCard>
      </section>
    </>
  )
}

function ServiceTable(props: {
  services: ServiceEntry[]
  agentMap: Map<string, string>
  busy: string | null
  onConnect: (service: ServiceEntry) => void
  onDelete: (service: ServiceEntry) => void
  onDisconnect: (service: ServiceEntry) => void
  onOpen: (service: ServiceEntry) => void
  onRestart: (service: ServiceEntry) => void
}) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Service</TableHead>
          <TableHead>Agent</TableHead>
          <TableHead>Transport</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Tools</TableHead>
          <TableHead className="text-right">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {props.services.map((service) => (
          <TableRow
            key={service.name}
            className="cursor-pointer"
            tabIndex={0}
            onClick={() => props.onOpen(service)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) return
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                props.onOpen(service)
              }
            }}
          >
            <TableCell>
              <button
                className="flex max-w-96 flex-col gap-1 text-left"
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  props.onOpen(service)
                }}
              >
                <span className="font-medium">{service.name}</span>
                <span className="truncate text-sm text-muted-foreground">{String(service.config?.description || "No description")}</span>
              </button>
            </TableCell>
            <TableCell>{props.agentMap.get(service.name) || service.agent_id || "store"}</TableCell>
            <TableCell>{service.transport || "-"}</TableCell>
            <TableCell><StatusBadge status={service.status} /></TableCell>
            <TableCell>{service.tools?.length || 0}</TableCell>
            <TableCell className="text-right">
              <div className="flex justify-end gap-2" onClick={(event) => event.stopPropagation()}>
                <Button variant="ghost" size="sm" onClick={() => props.onOpen(service)}>
                  Detail
                  <ArrowRightIcon data-icon="inline-end" />
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="ghost" size="icon" aria-label={`Actions for ${service.name}`}>
                      <MoreHorizontalIcon />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuGroup>
                      {service.status === "Connected" ? (
                        <DropdownMenuItem onClick={() => props.onDisconnect(service)}>Disconnect</DropdownMenuItem>
                      ) : (
                        <DropdownMenuItem onClick={() => props.onConnect(service)}>Connect</DropdownMenuItem>
                      )}
                      <DropdownMenuItem onClick={() => props.onRestart(service)}>Restart</DropdownMenuItem>
                      <DropdownMenuItem variant="destructive" onClick={() => props.onDelete(service)}>Delete</DropdownMenuItem>
                    </DropdownMenuGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}

function ToolCard({ tool, sourceLabel, onRun, onDetail }: { tool: ToolInfo; sourceLabel?: string; onRun: () => void; onDetail: () => void }) {
  const schema = getToolSchema(tool) as { properties?: Record<string, { type?: string; description?: string }>; required?: string[] }
  const params = Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))

  async function onCopy() {
    await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
    toast.success("Tool copied")
  }

  return (
    <PanelCard>
      <SectionHeading
        title={tool.name}
        titleAs="h2"
        description={tool.description || "No description"}
        className="border-b-0 pb-0"
        actions={
          <div className="flex flex-wrap justify-end gap-2">
            <Button size="sm" onClick={onRun}>
              <WrenchIcon data-icon="inline-start" />
              Run
            </Button>
            <Button size="sm" variant="outline" onClick={onDetail}>
              <EyeIcon data-icon="inline-start" />
              Details
            </Button>
            <Button size="sm" variant="outline" onClick={onCopy}>
              <ClipboardIcon data-icon="inline-start" />
              Copy
            </Button>
          </div>
        }
      />
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap gap-2">
          <Badge variant="secondary">{sourceLabel || getToolServiceName(tool) || "store"}</Badge>
          {schema.required?.length ? <Badge variant="outline">{schema.required.length} required</Badge> : <Badge variant="outline">optional</Badge>}
        </div>
        {params.length ? (
          params.slice(0, 4).map(([name, meta]) => (
            <EntityRow key={name} actions={<Badge variant="outline">{meta.type || "any"}</Badge>}>
              <div className="min-w-0">
                <code className="text-sm font-medium">{name}</code>
                <p className="truncate text-sm text-muted-foreground">{meta.description || "No description"}</p>
              </div>
            </EntityRow>
          ))
        ) : (
          <p className="text-sm text-muted-foreground">No params required</p>
        )}
      </div>
    </PanelCard>
  )
}

function AddServiceView({ agents, onBack, onAdded }: { agents: AgentItem[]; onBack: () => void; onAdded: () => Promise<void> }) {
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [transport, setTransport] = useState<"stdio" | "streamable-http" | "sse">("stdio")
  const [agentId, setAgentId] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const agentIds = agents.map(getAgentId).filter(Boolean)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    setSubmitting(true)
    try {
      await addService({
        name: String(data.get("name") || "").trim(),
        scope,
        agentId: scope === "agent" ? agentId || String(data.get("agentId") || "").trim() : undefined,
        transport,
        commandOrUrl: String(data.get("commandOrUrl") || "").trim(),
        description: String(data.get("description") || "").trim() || undefined,
        workingDir: String(data.get("workingDir") || "").trim() || undefined,
        env: parseKvLines(String(data.get("env") || "")),
        headers: parseKvLines(String(data.get("headers") || "")),
      })
      toast.success("Service added")
      await onAdded()
      onBack()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Add service failed")
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <>
      <DetailHeader eyebrow="添加服务" title="New MCP Service" actions={<Button variant="outline" onClick={onBack}>Back</Button>} />
      <PanelCard>
        <SectionHeading title="Service Config" titleAs="h2" className="border-b-0 pb-0" />
        <form onSubmit={onSubmit}>
          <FieldGroup>
              <div className="grid gap-4 md:grid-cols-2">
                <Field>
                  <FieldLabel htmlFor="name">Name</FieldLabel>
                  <Input id="name" name="name" placeholder="github" required />
                </Field>
                <Field>
                  <FieldLabel>Scope</FieldLabel>
                  <Select value={scope} onValueChange={(value) => setScope(value as "store" | "agent")}>
                    <SelectTrigger><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="store">Store</SelectItem>
                        <SelectItem value="agent">Agent</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <Field data-disabled={scope === "store"}>
                  <FieldLabel>Agent ID</FieldLabel>
                  {agentIds.length ? (
                    <Select value={agentId || "manual"} onValueChange={(value) => setAgentId(value === "manual" ? "" : value)} disabled={scope === "store"}>
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          <SelectItem value="manual">Manual</SelectItem>
                          {agentIds.map((id) => <SelectItem key={id} value={id}>{id}</SelectItem>)}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input name="agentId" placeholder="agent-a" disabled={scope === "store"} required={scope === "agent"} />
                  )}
                </Field>
                <Field>
                  <FieldLabel>Transport</FieldLabel>
                  <Select value={transport} onValueChange={(value) => setTransport(value as "stdio" | "streamable-http" | "sse")}>
                    <SelectTrigger><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="stdio">stdio</SelectItem>
                        <SelectItem value="streamable-http">streamable-http</SelectItem>
                        <SelectItem value="sse">sse</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
              </div>

              {scope === "agent" && agentIds.length ? (
                <Field>
                  <FieldLabel htmlFor="agentId">Manual Agent ID</FieldLabel>
                  <Input id="agentId" name="agentId" placeholder="agent-a" disabled={Boolean(agentId)} required={!agentId} />
                </Field>
              ) : null}

              <Field>
                <FieldLabel htmlFor="commandOrUrl">Command or URL</FieldLabel>
                <InputGroup>
                  <InputGroupAddon align="inline-start">{transport}</InputGroupAddon>
                  <InputGroupInput id="commandOrUrl" name="commandOrUrl" placeholder={transport === "stdio" ? "npx -y @modelcontextprotocol/server-filesystem ." : "https://example.com/mcp"} required />
                </InputGroup>
              </Field>

              <Field>
                <FieldLabel htmlFor="description">Description</FieldLabel>
                <Input id="description" name="description" placeholder="Optional description" />
              </Field>

              <Field>
                <FieldLabel htmlFor="workingDir">Working directory</FieldLabel>
                <InputGroup>
                  <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
                  <InputGroupInput id="workingDir" name="workingDir" placeholder="Optional" />
                </InputGroup>
              </Field>

              <Tabs defaultValue="env">
                <TabsList>
                  <TabsTrigger value="env">Env</TabsTrigger>
                  <TabsTrigger value="headers">Headers</TabsTrigger>
                </TabsList>
                <TabsContent value="env">
                  <Field>
                    <FieldLabel htmlFor="env">Env vars</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea id="env" name="env" placeholder="TOKEN=..." />
                    </InputGroup>
                  </Field>
                </TabsContent>
                <TabsContent value="headers">
                  <Field>
                    <FieldLabel htmlFor="headers">Headers</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea id="headers" name="headers" placeholder="Authorization=Bearer ..." />
                    </InputGroup>
                  </Field>
                </TabsContent>
              </Tabs>

              <div className="flex justify-end gap-2">
                <Button type="button" variant="outline" onClick={onBack}>Cancel</Button>
                <Button type="submit" disabled={submitting}>
                  {submitting ? <Spinner data-icon="inline-start" /> : null}
                  {submitting ? "Adding" : "Add Service"}
                </Button>
              </div>
          </FieldGroup>
        </form>
      </PanelCard>
    </>
  )
}

function RunToolDialog({ state, onOpenChange }: { state: ToolDialogState; onOpenChange: (open: boolean) => void }) {
  const [args, setArgs] = useState("{}")
  const [result, setResult] = useState<unknown>(null)
  const [running, setRunning] = useState(false)

  useEffect(() => {
    if (state) {
      setArgs("{}")
      setResult(null)
    }
  }, [state])

  async function onRun(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!state) return
    setRunning(true)
    try {
      const parsed = JSON.parse(args)
      if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") throw new Error("Args must be a JSON object")
      setResult(await state.onRun(parsed))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Tool call failed")
    } finally {
      setRunning(false)
    }
  }

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Run tool: {state?.tool.name}</DialogTitle>
          <DialogDescription>{state?.sourceLabel}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onRun}>
          <Field>
            <FieldLabel htmlFor="tool-args">Args JSON</FieldLabel>
            <InputGroup>
              <InputGroupTextarea id="tool-args" value={args} onChange={(event) => setArgs(event.target.value)} rows={6} />
            </InputGroup>
          </Field>
          {result ? <JsonBlock value={result} /> : null}
          <DialogFormFooter cancelLabel="Close" onCancel={() => onOpenChange(false)} submitLabel={running ? "Running" : "Run"} submitting={running} />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}

function ToolDetailDialog({ state, onOpenChange, onRun }: { state: ToolDetailState; onOpenChange: (open: boolean) => void; onRun: (state: NonNullable<ToolDetailState>) => void }) {
  const schema = state ? getToolSchema(state.tool) : {}

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (state?.onRun) onRun(state)
  }

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{state?.tool.name}</DialogTitle>
          <DialogDescription>{state?.sourceLabel}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onSubmit}>
          {state?.tool.description ? (
            <Field>
              <FieldLabel>Description</FieldLabel>
              <p className="text-sm text-muted-foreground">{state.tool.description}</p>
            </Field>
          ) : null}
          <Field>
            <FieldLabel>Param Schema</FieldLabel>
            <JsonBlock value={schema} />
          </Field>
          <Field>
            <FieldLabel>Raw Tool</FieldLabel>
            <JsonBlock value={state?.tool || {}} />
          </Field>
          <DialogFormFooter
            cancelLabel="Close"
            onCancel={() => onOpenChange(false)}
            submitButtonProps={{ className: state?.onRun ? undefined : "hidden" }}
            submitLabel="Run"
          />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}

function SwitchCacheDialog({ open, current, onOpenChange, onChanged }: { open: boolean; current?: CacheBackend; onOpenChange: (open: boolean) => void; onChanged: () => Promise<void> }) {
  const [target, setTarget] = useState<CacheBackend>(current || "memory")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open && current) setTarget(current)
  }, [current, open])

  async function onSwitch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    try {
      await switchCache(target)
      toast.success("Cache storage switched")
      await onChanged()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Switch failed")
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Switch cache storage</DialogTitle>
          <DialogDescription>Current cache storage: {current || "unknown"}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onSwitch}>
          <Field>
            <FieldLabel>Target cache storage</FieldLabel>
            <Select value={target} onValueChange={(value) => setTarget(value as CacheBackend)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {cacheOptions.map((option) => <SelectItem key={option} value={option}>{option}</SelectItem>)}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <DialogFormFooter cancelLabel="Cancel" onCancel={() => onOpenChange(false)} submitLabel={submitting ? "Switching" : "Switch"} submitting={submitting} />
        </DialogForm>
      </DialogContent>
    </Dialog>
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

function InfoCard({ label, value }: { label: string; value: string }) {
  return (
    <PanelCard variant="plain" className="flex min-w-0 flex-col gap-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <code className="truncate text-sm font-medium">{value}</code>
    </PanelCard>
  )
}

function SearchBox({ placeholder, value, onChange }: { placeholder: string; value: string; onChange: (value: string) => void }) {
  return (
    <InputGroup className="min-w-0 flex-1">
      <InputGroupAddon align="inline-start" className="pointer-events-none">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput placeholder={placeholder} value={value} onChange={(event) => onChange(event.target.value)} />
    </InputGroup>
  )
}

function StatusBadge({ status }: { status?: string }) {
  const label = status || "Unknown"
  return <Badge variant={label === "Connected" ? "default" : label === "Error" ? "destructive" : "secondary"}>{label}</Badge>
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <ScrollArea className="max-h-96 rounded-md bg-muted">
      <pre className="p-4 text-sm">{JSON.stringify(value, null, 2)}</pre>
    </ScrollArea>
  )
}

function getAgentId(agent: AgentItem) {
  return String(agent.agent_id || agent.id || "")
}

function getAgentServices(agent: AgentItem) {
  return (agent.services || agent.service_names || []).map(String)
}

function getToolSchema(tool: ToolInfo) {
  return tool.schema || tool.inputSchema || tool.input_schema || tool.parameters || {}
}

function getToolServiceName(tool: ToolInfo) {
  return String(tool.service_name || tool.service || tool.server_name || "") || undefined
}

function toolKey(tool: ToolInfo) {
  return `${getToolServiceName(tool) || "tool"}:${tool.name}`
}

function countKeys(value: unknown) {
  return value && typeof value === "object" ? Object.keys(value).length : 0
}
