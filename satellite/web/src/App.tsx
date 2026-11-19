import { useCallback, useMemo, useSyncExternalStore } from "react"
import { useAppActions } from "@/app/use-app-actions"
import { useAppConfirmations } from "@/app/use-app-confirmations"
import { useAppView } from "@/app/use-app-view"
import { AppDialogs } from "@/components/layout/app-dialogs"
import { AppBottomBar } from "@/components/layout/app-bottom-bar"
import { AppSidebar } from "@/components/layout/app-sidebar"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import {
  breadcrumbUppercase,
  type WorkspaceCrumb,
} from "@/components/shared/workspace-page-header"
import { useI18n } from "@/lib/i18n-context"
import { useAgentActions } from "@/features/agents/use-agent-actions"
import { ClientsView } from "@/features/clients/clients-view"
import { GatewayView } from "@/features/gateway/gateway-view"
import { OverviewView } from "@/features/overview/overview-view"
import { TraceSessionDetailView } from "@/features/traces/trace-session-detail-view"
import { TracesView } from "@/features/traces/traces-view"
import { ServiceDetailView } from "@/features/services/service-detail-view"
import { ServicesView } from "@/features/services/services-view"
import { useServiceActions } from "@/features/services/use-service-actions"
import { SettingsView } from "@/features/settings/settings-view"
import { UpdateBanner } from "@/features/settings/update-banner"
import { ToolDetailView } from "@/features/tools/tool-detail-view"
import { PromptDetailView } from "@/features/prompts/prompt-detail-view"
import { PromptsView } from "@/features/prompts/prompts-view"
import { ResourceDetailView } from "@/features/resources/resource-detail-view"
import { ResourcesView } from "@/features/resources/resources-view"
import { PluginsView } from "@/features/plugins/plugins-view"
import { ScopeView } from "@/features/scope/scope-view"
import { ToolsView } from "@/features/tools/tools-view"
import { useToolDialogState } from "@/features/tools/use-tool-dialog-state"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { getSelectedHost, onSelectedHostChanged } from "@/lib/api/backend"
import { useAppQueryRefreshers } from "@/hooks/use-app-query-refreshers"
import { useDashboard } from "@/hooks/use-dashboard"
import { useUiStore } from "@/stores/ui-store"

export function App() {
  const { t } = useI18n()
  const { services, agents, kvStore, loading, error: dashboardError, refresh } = useDashboard()
  // Remount resource views when the selected daemon changes: queries rebuild
  // host-scoped keys and view-local state (telemetry cursor, dialogs) resets.
  const selectedHost = useSyncExternalStore(onSelectedHostChanged, getSelectedHost)
  const {
    refreshAgentQueries,
    refreshKvQueries,
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
  const handleToolNameChange = useCallback(
    (toolName: string) => {
      if (view.name !== "tool") return
      if (view.toolName === toolName) return
      setView({ name: "tool", instanceId: view.instanceId, toolName }, { replace: true })
    },
    [setView, view],
  )
  const { busy, runAction } = useAppActions(refresh)
  const {
    closeToolDetail,
    closeToolResult,
    isToolRunning,
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    runToolFromDialogState,
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
  const kvDialogOpen = useUiStore((state) => state.kvDialogOpen)
  const setKvDialogOpen = useUiStore((state) => state.setKvDialogOpen)
  const {
    closeDeleteDialog,
    closeResetDialog,
    confirmReset,
    deleteTarget,
    resetTarget,
    setDeleteTarget,
  } = useAppConfirmations({ refreshConfigQueries, runAction })
  const handleServiceAdded = async () => {
    await refresh()
    await refreshServiceRegistryQueries()
  }

  const instanceTrailPrefix = useMemo((): WorkspaceCrumb[] | undefined => {
    if (view.name !== "instance" || view.origin?.kind !== "scope") return undefined
    const scopeId = view.origin.scopeId
    const scopeLabel =
      scopeId === "store"
        ? breadcrumbUppercase(t("store"))
        : scopeId === "all"
          ? breadcrumbUppercase(t("filterAll"))
          : breadcrumbUppercase(scopeId)
    return [
      { label: t("navAgents"), onClick: () => setView({ name: "scope" }) },
      { label: scopeLabel, onClick: () => setView({ name: "scope", scopeId }) },
    ]
  }, [setView, t, view])

  return (
    <TooltipProvider>
      <div className="flex h-dvh flex-col overflow-hidden bg-background">
        <div className="flex min-h-0 flex-1 overflow-hidden">
          <AppSidebar view={view} onViewChange={setView} />
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <main className="flex min-h-0 flex-1 flex-col overflow-hidden bg-muted">
              <div
                key={selectedHost}
                className="mx-auto flex min-h-0 w-full max-w-[1200px] min-w-0 flex-1 flex-col gap-3 px-3 pt-3 pb-2"
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
                      trailPrefix={instanceTrailPrefix}
                      onRunTool={(tool, args) => openServiceToolRunner(selectedService, tool, args)}
                      isToolRunning={(tool) =>
                        isToolRunning(selectedService.instance_id, tool.name)
                      }
                      onToolDetail={(tool, service, statusReport) =>
                        openServiceToolDetail(service, tool, statusReport)
                      }
                      onConnect={() => connectServiceEntry(selectedService)}
                      onDisconnect={() => disconnectServiceEntry(selectedService)}
                      onRestart={() => restartServiceEntry(selectedService)}
                      onDelete={() => setDeleteTarget(selectedService)}
                    />
                  ) : selectedServiceError ? (
                    <PageError
                      title="Instance failed to load"
                      message={
                        selectedServiceError instanceof Error
                          ? selectedServiceError.message
                          : String(selectedServiceError)
                      }
                      onRefresh={() => void refreshSelectedService()}
                    />
                  ) : selectedServiceLoading ? (
                    <PageSkeleton />
                  ) : null
                ) : view.name === "tools" ? (
                  <ToolsView
                    agents={agents}
                    services={services}
                    focusInstanceId={view.instanceId}
                    onFocusConsumed={() => {
                      if (view.name === "tools" && view.instanceId) {
                        setView({ name: "tools" }, { replace: true })
                      }
                    }}
                    onOpenTool={(instanceId, toolName) =>
                      setView({ name: "tool", instanceId, toolName })
                    }
                  />
                ) : view.name === "resources" ? (
                  <ResourcesView
                    agents={agents}
                    services={services}
                    onOpenResource={(instanceId, resourceUri) =>
                      setView({ name: "resource", instanceId, resourceUri })
                    }
                  />
                ) : view.name === "prompts" ? (
                  <PromptsView
                    agents={agents}
                    services={services}
                    onOpenPrompt={(instanceId, promptName) =>
                      setView({ name: "prompt", instanceId, promptName })
                    }
                  />
                ) : view.name === "tool" ? (
                  <ToolDetailView
                    instanceId={view.instanceId}
                    toolName={view.toolName}
                    services={services}
                    onBack={goBack}
                    onOpenTools={(instanceId) =>
                      setView(instanceId ? { name: "tools", instanceId } : { name: "tools" })
                    }
                    onRunTool={runToolFromDialogState}
                    onToolNameChange={handleToolNameChange}
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
                ) : view.name === "scope" ? (
                  <ScopeView
                    services={services}
                    agents={agents}
                    scopeId={view.scopeId}
                    onScopeIdChange={(scopeId) =>
                      setView(scopeId ? { name: "scope", scopeId } : { name: "scope" })
                    }
                    onOpenService={(service) =>
                      setView({
                        name: "instance",
                        addr: service,
                        origin:
                          view.scopeId != null
                            ? { kind: "scope", scopeId: view.scopeId }
                            : undefined,
                      })
                    }
                  />
                ) : view.name === "clients" ? (
                  <ClientsView />
                ) : view.name === "plugins" ? (
                  <PluginsView initialTab={view.tab} />
                ) : view.name === "gateway" ? (
                  <GatewayView
                    onOpenAgents={() => setView({ name: "clients" })}
                    onOpenScope={(scopeId) => setView({ name: "scope", scopeId })}
                  />
                ) : view.name === "traces" ? (
                  <TracesView
                    initialTab={view.tab}
                    onOpenSession={(session) => {
                      if (
                        session.face === "upstream" &&
                        session.instance_id &&
                        services.some((service) => service.instance_id === session.instance_id)
                      ) {
                        setView({
                          name: "services",
                          cardInstanceId: session.instance_id,
                          cardTab: "sessions",
                        })
                        return
                      }
                      setView({
                        name: "traceSession",
                        sessionId: session.session_id,
                        face: session.face,
                      })
                    }}
                  />
                ) : view.name === "traceSession" ? (
                  <TraceSessionDetailView
                    face={view.face}
                    sessionId={view.sessionId}
                    onBack={goBack}
                  />
                ) : view.name === "settings" ? (
                  <SettingsView />
                ) : (
                  <ServicesView
                    services={services}
                    agents={agents}
                    busy={busy}
                    error={dashboardError}
                    loading={loading}
                    openCardInstanceId={
                      view.name === "services" ? view.cardInstanceId : undefined
                    }
                    openCardTab={view.name === "services" ? view.cardTab : undefined}
                    onOpenCardConsumed={() => {
                      if (view.name === "services" && (view.cardInstanceId || view.cardTab)) {
                        setView({ name: "services" })
                      }
                    }}
                    onCheck={checkAllServices}
                    onConnect={connectServiceEntry}
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
        kvStore={kvStore}
        kvDialogOpen={kvDialogOpen}
        deleteTarget={deleteTarget}
        resetTarget={resetTarget}
        toolDetail={toolDetail}
        toolResult={toolResult}
        onKvChanged={async () => {
          await refresh()
          await refreshKvQueries()
        }}
        onKvDialogOpenChange={setKvDialogOpen}
        onConfirmDelete={(service) =>
          removeServiceEntry(service).then(() => setView({ name: "services" }))
        }
        onConfirmReset={confirmReset}
        onDeleteDialogOpenChange={closeDeleteDialog}
        onResetDialogOpenChange={closeResetDialog}
        onRunToolFromDetail={openToolRunnerFromDetail}
        onToolDetailOpenChange={closeToolDetail}
        onToolResultOpenChange={closeToolResult}
      />
      <Toaster />
      <UpdateBanner />
    </TooltipProvider>
  )
}
