import { useState } from "react"
import {
  ArrowLeftIcon,
  PlusIcon,
  RefreshCwIcon,
  SettingsIcon,
  Trash2Icon,
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
import { Button } from "@/components/ui/button"
import { AgentsView } from "@/features/agents/agents-view"
import { CacheView } from "@/features/cache/cache-view"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { ConfigView, type ResetTarget } from "@/features/config/config-view"
import { AddServiceView } from "@/features/services/add-service-view"
import { ServiceDetailView } from "@/features/services/service-detail-view"
import { ServicesView } from "@/features/services/services-view"
import { SettingsDialog } from "@/features/settings/settings-dialog"
import { RunToolDialog, ToolDetailDialog, type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { ToolsView } from "@/features/tools/tools-view"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useDashboard } from "@/hooks/use-dashboard"
import { useUiStore } from "@/stores/ui-store"
import {
  assignService,
  callTool,
  checkServices,
  connectService,
  disconnectService,
  removeService,
  resetAgentConfig,
  resetConfig,
  restartService,
  unassignService,
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
