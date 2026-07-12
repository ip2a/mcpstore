import { useAppActions } from "@/app/use-app-actions"
import { useAppConfirmations } from "@/app/use-app-confirmations"
import { useAppView } from "@/app/use-app-view"
import { AppDialogs } from "@/components/layout/app-dialogs"
import { AppHeader } from "@/components/layout/app-header"
import { AgentsView } from "@/features/agents/agents-view"
import { useAgentActions } from "@/features/agents/use-agent-actions"
import { CacheView } from "@/features/cache/cache-view"
import { ConfigView } from "@/features/config/config-view"
import { ServiceDetailView } from "@/features/services/service-detail-view"
import { ServicesView } from "@/features/services/services-view"
import { useServiceActions } from "@/features/services/use-service-actions"
import { ToolsView } from "@/features/tools/tools-view"
import { useToolDialogState } from "@/features/tools/use-tool-dialog-state"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useAppQueryRefreshers } from "@/hooks/use-app-query-refreshers"
import { useDashboard } from "@/hooks/use-dashboard"
import { useUiStore } from "@/stores/ui-store"
import { cn } from "@/lib/utils"

export function App() {
  const { services, agents, agentMap, backend, loading, error: dashboardError, refresh } = useDashboard()
  const {
    cacheRevision,
    refreshAgentQueries,
    refreshCacheQueries,
    refreshConfigQueries,
    refreshServiceQueries,
    refreshServiceRegistryQueries,
    serviceDetailRevision,
  } = useAppQueryRefreshers()
  const { goBack, pageTitle, selectedService, setView, view } = useAppView(services)
  const { busy, runAction } = useAppActions(refresh)
  const {
    closeToolDetail,
    closeToolDialog,
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    setToolDetail,
    setToolDialog,
    toolDetail,
    toolDialog,
  } = useToolDialogState()
  const {
    checkAllServices,
    connectServiceEntry,
    disconnectServiceEntry,
    removeServiceEntry,
    restartServiceEntry,
  } = useServiceActions({ refreshServiceQueries, runAction })
  const { assignServiceToAgent, unassignServiceFromAgent } = useAgentActions({ refreshAgentQueries, refreshServiceQueries, runAction })
  const cacheDialogOpen = useUiStore((state) => state.cacheDialogOpen)
  const setCacheDialogOpen = useUiStore((state) => state.setCacheDialogOpen)
  const settingsDialogOpen = useUiStore((state) => state.settingsDialogOpen)
  const setSettingsDialogOpen = useUiStore((state) => state.setSettingsDialogOpen)
  const { closeDeleteDialog, closeResetDialog, confirmReset, deleteTarget, resetTarget, setDeleteTarget, setResetTarget } = useAppConfirmations({ refreshConfigQueries, runAction })
  const handleServiceAdded = async () => {
    await refresh()
    await refreshServiceRegistryQueries()
  }

  return (
    <TooltipProvider>
      <div className="min-h-dvh bg-background">
        <div className="mx-auto grid h-dvh w-[min(1280px,calc(100vw-24px))] grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
          <AppHeader
            agents={agents}
            pageTitle={pageTitle}
            view={view}
            onAdded={handleServiceAdded}
            onBack={goBack}
            onViewChange={setView}
            onOpenSettings={() => setSettingsDialogOpen(true)}
          />

          <main
            className={cn(
              "flex min-h-0 flex-col",
              selectedService || view.name === "services" || view.name === "agents" || view.name === "cache" || view.name === "config" || view.name === "tools" ? "h-full overflow-hidden gap-3 pt-3" : "gap-6 overflow-auto py-3",
            )}
          >
          {selectedService ? (
            <ServiceDetailView
              service={selectedService}
              busy={busy}
              refreshToken={serviceDetailRevision}
              onBack={goBack}
              onRunTool={(tool, args) => openServiceToolRunner(selectedService, tool, args)}
              onToolDetail={(tool, service, statusReport) => openServiceToolDetail(service, tool, statusReport)}
              onConnect={() => connectServiceEntry(selectedService)}
              onDisconnect={() => disconnectServiceEntry(selectedService)}
              onRestart={() => restartServiceEntry(selectedService)}
              onDelete={() => setDeleteTarget(selectedService)}
            />
          ) : view.name === "agents" ? (
            <AgentsView
              agents={agents}
              services={services}
              loading={loading}
              busy={busy}
              onRefresh={refresh}
              onAssign={assignServiceToAgent}
              onOpenService={(serviceName) => setView({ name: "service", serviceName })}
              onUnassign={unassignServiceFromAgent}
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
            <CacheView backend={backend} revision={cacheRevision} onRefreshDashboard={refresh} onSwitch={() => setCacheDialogOpen(true)} />
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
              onCheck={checkAllServices}
              onConnect={connectServiceEntry}
              onDelete={setDeleteTarget}
              onDisconnect={disconnectServiceEntry}
              onOpen={(service) => setView({ name: "service", serviceName: service.name })}
              onRefresh={refresh}
              onRestart={restartServiceEntry}
            />
          )}
          </main>
        </div>
      </div>

      <AppDialogs
        backend={backend}
        cacheDialogOpen={cacheDialogOpen}
        deleteTarget={deleteTarget}
        resetTarget={resetTarget}
        settingsDialogOpen={settingsDialogOpen}
        toolDetail={toolDetail}
        toolDialog={toolDialog}
        onCacheChanged={async () => {
          await refresh()
          await refreshCacheQueries()
        }}
        onCacheDialogOpenChange={setCacheDialogOpen}
        onConfirmDelete={(service) => removeServiceEntry(service).then(() => setView({ name: "services" }))}
        onConfirmReset={confirmReset}
        onDeleteDialogOpenChange={closeDeleteDialog}
        onResetDialogOpenChange={closeResetDialog}
        onRunToolFromDetail={openToolRunnerFromDetail}
        onSettingsDialogOpenChange={setSettingsDialogOpen}
        onToolDetailOpenChange={closeToolDetail}
        onToolDialogOpenChange={closeToolDialog}
      />
      <Toaster />
    </TooltipProvider>
  )
}
