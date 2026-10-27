import { useAppActions } from "@/app/use-app-actions"
import { useAppConfirmations } from "@/app/use-app-confirmations"
import { useAppView } from "@/app/use-app-view"
import { AppDialogs } from "@/components/layout/app-dialogs"
import { AppBottomBar } from "@/components/layout/app-bottom-bar"
import { AppSidebar } from "@/components/layout/app-sidebar"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { useAgentActions } from "@/features/agents/use-agent-actions"
import { CacheView } from "@/features/cache/cache-view"
import { ClientsView } from "@/features/clients/clients-view"
import { GatewayView } from "@/features/gateway/gateway-view"
import { OverviewView } from "@/features/overview/overview-view"
import { ServiceDetailView } from "@/features/services/service-detail-view"
import { ServicesView } from "@/features/services/services-view"
import { useServiceActions } from "@/features/services/use-service-actions"
import { SettingsView } from "@/features/settings/settings-view"
import { ToolDetailView } from "@/features/tools/tool-detail-view"
import { PromptDetailView } from "@/features/prompts/prompt-detail-view"
import { PromptsView } from "@/features/prompts/prompts-view"
import { ResourceDetailView } from "@/features/resources/resource-detail-view"
import { ResourcesView } from "@/features/resources/resources-view"
import { PluginsView } from "@/features/plugins/plugins-view"
import { ToolsView } from "@/features/tools/tools-view"
import { useToolDialogState } from "@/features/tools/use-tool-dialog-state"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useAppQueryRefreshers } from "@/hooks/use-app-query-refreshers"
import { useDashboard } from "@/hooks/use-dashboard"
import { useUiStore } from "@/stores/ui-store"
import { cn } from "@/lib/utils"

export function App() {
  const { services, agents, backend, loading, error: dashboardError, refresh } = useDashboard()
  const {
    cacheRevision,
    refreshAgentQueries,
    refreshCacheQueries,
    refreshConfigQueries,
    refreshInstanceQueries,
    refreshServiceRegistryQueries,
    serviceDetailRevision,
  } = useAppQueryRefreshers()
  const {
    goBack,
    refreshSelectedService,
    selectedService,
    selectedServiceError,
    selectedServiceLoading,
    setView,
    view,
  } = useAppView(services)
  const { busy, runAction } = useAppActions(refresh)
  const {
    closeToolDetail,
    closeToolDialog,
    closeToolResult,
    isToolRunning,
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    runToolFromDialogState,
    setToolDetail,
    toolDetail,
    toolResult,
  } = useToolDialogState()
  const {
    checkAllServices,
    connectServiceEntry,
    disconnectServiceEntry,
    removeServiceEntry,
    restartServiceEntry,
  } = useServiceActions({ refreshInstanceQueries, runAction, services })
  const { declareServiceScope } = useAgentActions({
    refreshAgentQueries,
    refreshServiceRegistryQueries,
    runAction,
  })
  const cacheDialogOpen = useUiStore((state) => state.cacheDialogOpen)
  const setCacheDialogOpen = useUiStore((state) => state.setCacheDialogOpen)
  const { closeDeleteDialog, closeResetDialog, confirmReset, deleteTarget, resetTarget, setDeleteTarget, setResetTarget } = useAppConfirmations({ refreshConfigQueries, runAction })
  const handleServiceAdded = async () => {
    await refresh()
    await refreshServiceRegistryQueries()
  }

  return (
    <TooltipProvider>
      <div className="flex h-dvh flex-col overflow-hidden bg-background">
        <div className="flex min-h-0 flex-1 overflow-hidden">
          <AppSidebar view={view} onViewChange={setView} />
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <main className="flex min-h-0 flex-1 flex-col overflow-hidden bg-muted">
            <div
              className={cn(
                "mx-auto flex min-h-0 w-full max-w-[1200px] min-w-0 flex-1 flex-col px-3",
                view.name === "instance" || view.name === "home" || view.name === "services" || view.name === "clients" || view.name === "plugins" || view.name === "gateway" || view.name === "cache" || view.name === "settings" || view.name === "tools" || view.name === "tool" || view.name === "resources" || view.name === "resource" || view.name === "prompts" || view.name === "prompt" ? "gap-3 pt-3 pb-2" : "gap-6 overflow-auto py-3",
              )}
            >
            {view.name === "home" ? (
              <OverviewView />
            ) : view.name === "instance" ? (
            selectedService ? (
              <ServiceDetailView
                service={selectedService}
                busy={busy}
                refreshToken={serviceDetailRevision}
                onBack={goBack}
                onRunTool={(tool, args) => openServiceToolRunner(selectedService, tool, args)}
                isToolRunning={(tool) => isToolRunning(selectedService.instance_id, tool.name)}
                onToolDetail={(tool, service, statusReport) => openServiceToolDetail(service, tool, statusReport)}
                onConnect={() => connectServiceEntry(selectedService)}
                onDisconnect={() => disconnectServiceEntry(selectedService)}
                onRestart={() => restartServiceEntry(selectedService)}
                onDelete={() => setDeleteTarget(selectedService)}
              />
            ) : selectedServiceError ? (
              <PageError
                title="Instance failed to load"
                message={selectedServiceError instanceof Error ? selectedServiceError.message : String(selectedServiceError)}
                onRefresh={() => void refreshSelectedService()}
              />
            ) : selectedServiceLoading ? (
              <PageSkeleton />
            ) : null
          ) : view.name === "tools" ? (
            <ToolsView
              agents={agents}
              services={services}
              onOpenTool={(instanceId, toolName) => setView({ name: "tool", instanceId, toolName })}
            />
          ) : view.name === "resources" ? (
            <ResourcesView
              agents={agents}
              services={services}
              onOpenResource={(instanceId, resourceUri) => setView({ name: "resource", instanceId, resourceUri })}
            />
          ) : view.name === "prompts" ? (
            <PromptsView
              agents={agents}
              services={services}
              onOpenPrompt={(instanceId, promptName) => setView({ name: "prompt", instanceId, promptName })}
            />
          ) : view.name === "tool" ? (
            <ToolDetailView
              instanceId={view.instanceId}
              toolName={view.toolName}
              services={services}
              onBack={goBack}
              onRunTool={runToolFromDialogState}
              isToolRunning={(instanceId, toolName) => isToolRunning(instanceId, toolName)}
            />
          ) : view.name === "resource" ? (
            <ResourceDetailView
              instanceId={view.instanceId}
              resourceUri={view.resourceUri}
              services={services}
              onBack={goBack}
            />
          ) : view.name === "prompt" ? (
            <PromptDetailView
              instanceId={view.instanceId}
              promptName={view.promptName}
              services={services}
              onBack={goBack}
            />
          ) : view.name === "clients" ? (
            <ClientsView />
          ) : view.name === "plugins" ? (
            <PluginsView />
          ) : view.name === "gateway" ? (
            <GatewayView />
          ) : view.name === "cache" ? (
            <CacheView backend={backend} revision={cacheRevision} onRefreshDashboard={refresh} onSwitch={() => setCacheDialogOpen(true)} />
          ) : view.name === "settings" ? (
            <SettingsView />
          ) : (
            <ServicesView
              services={services}
              agents={agents}
              busy={busy}
              error={dashboardError}
              loading={loading}
              onCache={() => setView({ name: "cache" })}
              onCheck={checkAllServices}
              onConnect={connectServiceEntry}
              onDeclareScope={declareServiceScope}
              onDelete={setDeleteTarget}
              onDisconnect={disconnectServiceEntry}
              onOpen={(service) => setView({ name: "instance", addr: service })}
              onRefresh={refresh}
              onRestart={restartServiceEntry}
              onAdded={handleServiceAdded}
            />
          )}
            </div>
            </main>
          </div>
        </div>
        <AppBottomBar />
      </div>

      <AppDialogs
        backend={backend}
        cacheDialogOpen={cacheDialogOpen}
        deleteTarget={deleteTarget}
        resetTarget={resetTarget}
        toolDetail={toolDetail}
        toolDialog={null}
        toolResult={toolResult}
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
        onToolDetailOpenChange={closeToolDetail}
        onToolDialogOpenChange={closeToolDialog}
        onToolResultOpenChange={closeToolResult}
      />
      <Toaster />
    </TooltipProvider>
  )
}
