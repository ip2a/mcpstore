import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"
import { AppHeader, type AppView, viewTitle } from "@/components/layout/app-header"
import { AgentsView } from "@/features/agents/agents-view"
import { CacheView } from "@/features/cache/cache-view"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { ConfigView, type ResetTarget } from "@/features/config/config-view"
import { ResetConfigDialog } from "@/features/config/reset-config-dialog"
import { AddServiceView } from "@/features/services/add-service-view"
import { DeleteServiceDialog } from "@/features/services/delete-service-dialog"
import { ServiceDetailView } from "@/features/services/service-detail-view"
import { ServicesView } from "@/features/services/services-view"
import { SettingsDialog } from "@/features/settings/settings-dialog"
import { RunToolDialog, ToolDetailDialog, type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { ToolsView } from "@/features/tools/tools-view"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useDashboard } from "@/hooks/use-dashboard"
import { queryKeys } from "@/lib/query-keys"
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

export function App() {
  const { services, agents, agentMap, backend, loading, error: dashboardError, refresh } = useDashboard()
  const queryClient = useQueryClient()
  const [view, setView] = useState<AppView>({ name: "services" })
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)
  const [cacheDialog, setCacheDialog] = useState(false)
  const settingsDialogOpen = useUiStore((state) => state.settingsDialogOpen)
  const setSettingsDialogOpen = useUiStore((state) => state.setSettingsDialogOpen)
  const [deleteTarget, setDeleteTarget] = useState<ServiceEntry | null>(null)
  const [resetTarget, setResetTarget] = useState<ResetTarget | null>(null)
  const [cacheRevision, setCacheRevision] = useState(0)
  const [serviceDetailRevision, setServiceDetailRevision] = useState(0)
  const [busy, setBusy] = useState<string | null>(null)

  const selectedService = view.name === "service" ? services.find((service) => service.name === view.serviceName) : undefined
  const pageTitle = viewTitle(view)

  async function runAction(label: string, action: () => Promise<unknown>, onSuccess?: () => Promise<void> | void) {
    setBusy(label)
    try {
      await action()
      toast.success("操作已完成")
      await refresh()
      await onSuccess?.()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败")
    } finally {
      setBusy(null)
    }
  }

  async function refreshServiceQueries(serviceName: string, agentId?: string) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.services }),
      queryClient.invalidateQueries({ queryKey: queryKeys.service(serviceName) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.serviceStatus(serviceName) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
      agentId ? queryClient.invalidateQueries({ queryKey: queryKeys.agent(agentId) }) : Promise.resolve(),
    ])
    setServiceDetailRevision((value) => value + 1)
  }

  async function refreshServiceRegistryQueries() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.services }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
    ])
  }

  async function refreshAgentQueries(agentId: string) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.agents }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agent(agentId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.toolsRoot }),
      queryClient.invalidateQueries({ queryKey: queryKeys.agentToolsRoot(agentId) }),
    ])
  }

  async function refreshCacheQueries() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.health }),
      queryClient.invalidateQueries({ queryKey: queryKeys.cacheHealth }),
      queryClient.invalidateQueries({ queryKey: queryKeys.cacheInspect }),
    ])
  }

  async function refreshConfigQueries(target: ResetTarget) {
    await queryClient.invalidateQueries({ queryKey: target.scope === "store" ? queryKeys.config : queryKeys.agentConfig(target.agentId) })
  }

  async function confirmReset(target: ResetTarget) {
    if (target.scope === "store") {
      await runAction("reset:store", resetConfig, () => refreshConfigQueries(target))
    } else {
      await runAction(`reset:${target.agentId}`, () => resetAgentConfig(target.agentId), () => refreshConfigQueries(target))
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
          <AppHeader pageTitle={pageTitle} view={view} onViewChange={setView} onOpenSettings={() => setSettingsDialogOpen(true)} />

          <main className="flex min-h-0 flex-col gap-6 overflow-auto py-3">
          {view.name === "add" ? (
            <AddServiceView agents={agents} onBack={() => setView({ name: "services" })} onAdded={async () => {
              await refresh()
              await refreshServiceRegistryQueries()
            }} />
          ) : selectedService ? (
            <ServiceDetailView
              service={selectedService}
              busy={busy}
              refreshToken={serviceDetailRevision}
              onBack={() => setView({ name: "services" })}
              onRunTool={(tool) => openServiceToolRunner(selectedService, tool)}
              onToolDetail={(tool) => openServiceToolDetail(selectedService, tool)}
              onConnect={() => runAction(`connect:${selectedService.name}`, () => connectService(selectedService.name), () => refreshServiceQueries(selectedService.name, selectedService.agent_id))}
              onDisconnect={() => runAction(`disconnect:${selectedService.name}`, () => disconnectService(selectedService.name), () => refreshServiceQueries(selectedService.name, selectedService.agent_id))}
              onRestart={() => runAction(`restart:${selectedService.name}`, () => restartService(selectedService.name), () => refreshServiceQueries(selectedService.name, selectedService.agent_id))}
              onDelete={() => setDeleteTarget(selectedService)}
            />
          ) : view.name === "agents" ? (
            <AgentsView
              agents={agents}
              services={services}
              loading={loading}
              busy={busy}
              onRefresh={refresh}
              onAssign={(agentId, serviceName) => runAction(`assign:${agentId}:${serviceName}`, () => assignService(agentId, serviceName), async () => {
                await Promise.all([refreshAgentQueries(agentId), refreshServiceQueries(serviceName, agentId)])
              })}
              onOpenService={(serviceName) => setView({ name: "service", serviceName })}
              onUnassign={(agentId, serviceName) => runAction(`unassign:${agentId}:${serviceName}`, () => unassignService(agentId, serviceName), async () => {
                await Promise.all([refreshAgentQueries(agentId), refreshServiceQueries(serviceName, agentId)])
              })}
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
              onConnect={(service) => runAction(`connect:${service.name}`, () => connectService(service.name), () => refreshServiceQueries(service.name, service.agent_id))}
              onDelete={setDeleteTarget}
              onDisconnect={(service) => runAction(`disconnect:${service.name}`, () => disconnectService(service.name), () => refreshServiceQueries(service.name, service.agent_id))}
              onOpen={(service) => setView({ name: "service", serviceName: service.name })}
              onRefresh={refresh}
              onRestart={(service) => runAction(`restart:${service.name}`, () => restartService(service.name), () => refreshServiceQueries(service.name, service.agent_id))}
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
          await refreshCacheQueries()
          setCacheRevision((value) => value + 1)
        }}
      />
      <SettingsDialog open={settingsDialogOpen} onOpenChange={setSettingsDialogOpen} />
      <DeleteServiceDialog
        service={deleteTarget}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        onConfirm={(service) => runAction(`delete:${service.name}`, () => removeService(service.name), () => refreshServiceQueries(service.name, service.agent_id)).then(() => setView({ name: "services" }))}
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
