import { useState, useMemo } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { PlusIcon } from "lucide-react"
import { toast } from "sonner"

import { AgentIdPicker } from "@/components/shared/agent-id-picker"
import { FlutedGlassBackground } from "@/components/shared/fluted-glass-background"
import { UiverseSearchInput } from "@/components/shared/uiverse-search-input"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { ServiceListCard } from "@/components/shared/service-list-card"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import {
  breadcrumbUppercase,
  WorkspaceBreadcrumb,
  WorkspacePageHeader,
} from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Spinner } from "@/components/ui/spinner"
import { useScopeServicesQuery, useScopesQuery } from "@/features/agents/queries"
import { getAgentId } from "@/features/agents/model"
import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"
import { serviceListLiveQueryOptions } from "@/features/services/service-live-query"
import {
  createAgentShell,
  type AgentItem,
  type ScopeSummary,
  type ScopeView,
  type ServiceInstance,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const SCOPE_STATUS_ORDER: ServiceDisplayStatus[] = [
  "connected",
  "disconnected",
  "error",
  "connecting",
]

const SCOPE_STATUS_COLOR: Record<ServiceDisplayStatus, string> = {
  connected: "text-emerald-200",
  connecting: "text-amber-200",
  disconnected: "text-white/70",
  error: "text-red-200",
}

const SCOPE_LIST_GRID_CLASSNAME =
  "grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"

function scopeIdFromSummary(summary: ScopeSummary): string {
  return summary.scope.type === "agent" ? summary.scope.agent_id : summary.scope.type
}

function scopeTitleFromSummary(summary: ScopeSummary, t: (key: string) => string): string {
  if (summary.scope.type === "all") return "all"
  if (summary.scope.type === "store") return t("store")
  if (summary.scope.type === "agent") return summary.scope.agent_id
  return scopeIdFromSummary(summary)
}

function scopeViewFromId(scopeId: string): ScopeView {
  if (scopeId === "all") return { type: "all" }
  if (scopeId === "store") return { type: "store" }
  return { type: "agent", agent_id: scopeId }
}

function servicesForScope(
  services: ServiceInstance[],
  scope: ScopeView,
): ServiceInstance[] {
  if (scope.type === "store") {
    return services.filter((service) => service.scope.type === "store")
  }
  if (scope.type === "agent") {
    return services.filter(
      (service) =>
        service.scope.type === "agent" && service.scope.agent_id === scope.agent_id,
    )
  }
  return services
}

export function ScopeView(props: {
  services: ServiceInstance[]
  agents?: AgentItem[]
  scopeId?: string
  onScopeIdChange?: (scopeId: string | null) => void
  onOpenService?: (service: ServiceInstance) => void
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const scopesQuery = useScopesQuery()
  const scopes = useMemo(
    () => (scopesQuery.data || []).filter((summary) => summary.scope.type !== "all"),
    [scopesQuery.data],
  )
  const [localScopeId, setLocalScopeId] = useState<string | null>(props.scopeId ?? null)
  const selectedScopeId = props.onScopeIdChange ? (props.scopeId ?? null) : localScopeId
  const setSelectedScopeId = props.onScopeIdChange ?? setLocalScopeId
  const [searchQuery, setSearchQuery] = useState("")
  const [addScopeOpen, setAddScopeOpen] = useState(false)
  const existingAgentIds = useMemo(
    () => [
      ...new Set([
        ...(props.agents || []).map(getAgentId).filter(Boolean),
        ...scopes
          .filter((summary) => summary.scope.type === "agent")
          .map((summary) => (summary.scope.type === "agent" ? summary.scope.agent_id : ""))
          .filter(Boolean),
      ]),
    ],
    [props.agents, scopes],
  )

  const selectedScope: ScopeView | null = useMemo(
    () => (selectedScopeId ? scopeViewFromId(selectedScopeId) : null),
    [selectedScopeId],
  )

  const liveServicesQuery = useScopeServicesQuery(selectedScope ?? { type: "all" }, {
    enabled: Boolean(selectedScope),
    ...serviceListLiveQueryOptions,
  })

  const scopeServices = selectedScope ? liveServicesQuery.data || [] : []

  const loading = scopesQuery.isFetching || (Boolean(selectedScope) && liveServicesQuery.isFetching)
  const error = scopesQuery.error || liveServicesQuery.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : "Failed to load scope"

  // List payload already embeds tools[] — no per-service /tools/list fan-out.
  const filteredServices = useMemo(() => {
    const needle = searchQuery.trim().toLowerCase()
    if (!needle) return scopeServices

    return scopeServices.filter((service) => {
      if (service.service_name.toLowerCase().includes(needle)) return true
      return (service.tools || []).some((tool) => {
        const displayName = tool.title?.trim() || tool.name
        const text = `${displayName} ${tool.description} ${tool.name}`.toLowerCase()
        return text.includes(needle)
      })
    })
  }, [scopeServices, searchQuery])

  const selectedSummary = scopes.find((summary) => scopeIdFromSummary(summary) === selectedScopeId)
  const selectedTitle = selectedSummary
    ? breadcrumbUppercase(scopeTitleFromSummary(selectedSummary, t))
    : ""

  async function handleRefresh() {
    // refetch() ignores enabled:false — only pull services when the view is fetchable.
    const canRefetchServices =
      selectedScope != null &&
      (selectedScope.type !== "agent" || Boolean(selectedScope.agent_id))
    await Promise.all([
      scopesQuery.refetch(),
      canRefetchServices ? liveServicesQuery.refetch() : Promise.resolve(),
    ])
  }

  function backToOverview() {
    setSelectedScopeId(null)
    setSearchQuery("")
  }

  return (
    <>
      {selectedScope ? (
        <WorkspaceBreadcrumb
          ariaLabel={t("navAgents")}
          subtitle={t("scopeConfigurationDescription")}
          crumbs={[
            {
              label: t("navAgents"),
              onClick: backToOverview,
            },
            { label: selectedTitle },
          ]}
        />
      ) : (
        <WorkspacePageHeader
          title={<h1 className="text-lg font-semibold">{t("navAgents")}</h1>}
          subtitle={t("allScopesDescription")}
        />
      )}

      <PanelCard className="flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        {/* Mirror tools Tabs: default `gap-2` between toolbar and content + content `py-2` */}
        <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden">
          <div className={cn(WORKSPACE_TOOLBAR_CLASSNAME, "justify-end")}>
            {/* h-9 matches tools TabsList so toolbar height stays aligned */}
            <div className="flex h-9 min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              {selectedScope ? (
                <UiverseSearchInput
                  id="scope-list-search"
                  placeholder={t("searchToolsAndServices")}
                  value={searchQuery}
                  onChange={setSearchQuery}
                />
              ) : null}
              <Button size="sm" variant="outline" onClick={handleRefresh} disabled={loading}>
                <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
                {t("refresh")}
              </Button>
            </div>
          </div>

          <ScrollPane
            className="min-h-0 flex-1"
            innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, SCOPE_LIST_GRID_CLASSNAME)}
          >
            {error ? (
              <PageError
                className="col-span-full"
                title={t("scopeFailedToLoad")}
                message={errorMessage}
                onRefresh={handleRefresh}
              />
            ) : !selectedScope ? (
              scopesQuery.isFetching && !scopes.length ? (
                <PageSkeleton className="col-span-full" />
              ) : (
                <>
                  {!scopes.length ? (
                    <PageEmpty
                      className="col-span-full"
                      title={t("noScopes")}
                      description={t("noScopesDescription")}
                    />
                  ) : (
                    scopes.map((summary) => {
                      const scopeId = scopeIdFromSummary(summary)
                      const title = scopeTitleFromSummary(summary, t)
                      const scoped = servicesForScope(props.services, summary.scope)
                      return (
                        <ScopeListCard
                          key={scopeId}
                          title={title}
                          services={scoped}
                          onSelect={() => setSelectedScopeId(scopeId)}
                        />
                      )
                    })
                  )}
                  <AddScopeCard onSelect={() => setAddScopeOpen(true)} />
                </>
              )
            ) : loading && !scopeServices.length ? (
              <PageSkeleton className="col-span-full" />
            ) : !filteredServices.length ? (
              <div className="col-span-full flex h-full items-center justify-center p-8 text-center text-muted-foreground">
                {searchQuery ? t("noSearchResults") : t("noServices")}
              </div>
            ) : (
              filteredServices.map((service) => (
                <ServiceListCard
                  key={service.instance_id}
                  service={service}
                  onSelect={() => props.onOpenService?.(service)}
                />
              ))
            )}
          </ScrollPane>
        </div>
      </PanelCard>

      <AddAgentShellDialog
        agentIds={existingAgentIds}
        open={addScopeOpen}
        onOpenChange={setAddScopeOpen}
        onCreated={async (agentId) => {
          await client.invalidateQueries({ queryKey: queryKeys.scopes })
          setSelectedScopeId(agentId)
        }}
      />
    </>
  )
}

function AddScopeCard({ onSelect }: { onSelect: () => void }) {
  const { t } = useI18n()
  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex min-h-[9.5rem] cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <span className="flex size-8 items-center justify-center rounded-md border border-dashed border-border/70">
        <PlusIcon className="size-4" />
      </span>
      <span className="text-sm font-medium">{t("addScope")}</span>
    </div>
  )
}

function AddAgentShellDialog({
  agentIds,
  open,
  onOpenChange,
  onCreated,
}: {
  agentIds: string[]
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated: (agentId: string) => Promise<void>
}) {
  const { t } = useI18n()
  const [agentId, setAgentId] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const trimmed = agentId.trim()
  const canSubmit = Boolean(trimmed) && !submitting

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!trimmed) return
    setSubmitting(true)
    try {
      await createAgentShell(trimmed)
      toast.success(t("saved"))
      onOpenChange(false)
      setAgentId("")
      await onCreated(trimmed)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next)
        if (!next) setAgentId("")
      }}
    >
      <DialogContent className="sm:max-w-md">
        <form onSubmit={(event) => void onSubmit(event)}>
          <DialogHeader>
            <DialogTitle>{t("addScope")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <Field>
              <FieldLabel htmlFor="add-agent-shell-id">{t("agentId")}</FieldLabel>
              <AgentIdPicker
                id="add-agent-shell-id"
                agentIds={agentIds}
                value={agentId}
                onChange={setAgentId}
              />
              <p className="mt-1 text-xs text-muted-foreground">{t("addAgentShellHint")}</p>
            </Field>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("cancel")}
            </Button>
            <Button type="submit" disabled={!canSubmit}>
              {submitting ? <Spinner data-icon="inline-start" /> : <PlusIcon data-icon="inline-start" />}
              {submitting ? t("adding") : t("addScope")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function ScopeListCard({
  title,
  services,
  onSelect,
}: {
  title: string
  services: ServiceInstance[]
  onSelect: () => void
}) {
  const { t } = useI18n()
  const statusCounts = useMemo(() => {
    const counts: Record<ServiceDisplayStatus, number> = {
      connected: 0,
      connecting: 0,
      disconnected: 0,
      error: 0,
    }
    for (const service of services) {
      counts[deriveServiceDisplayStatus(service.state)] += 1
    }
    return SCOPE_STATUS_ORDER.flatMap((status) => {
      const count = counts[status]
      if (count <= 0) return []
      return [{ status, count }]
    })
  }, [services])

  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "group relative z-[1] flex h-[9.5rem] cursor-pointer flex-col overflow-hidden rounded-lg border border-white/15 bg-[#073d69] p-4 text-center",
        "shadow-[0_14px_28px_-20px_rgba(2,8,32,0.55)] transition-[border-color,box-shadow] duration-[400ms]",
        "hover:z-10 hover:border-white/35 hover:shadow-[0_18px_36px_-18px_rgba(2,8,32,0.7)]",
      )}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <FlutedGlassBackground />
      <div className="relative z-[1] flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-1 drop-shadow-[0_1px_3px_rgba(0,19,46,0.35)]">
        <span className="line-clamp-3 font-mono text-xl font-bold leading-snug tracking-tight text-white">
          {title}
        </span>
        {statusCounts.length ? (
          <p className="line-clamp-2 text-xs lowercase leading-5">
            {statusCounts.map(({ status, count }, index) => (
              <span key={status}>
                {index > 0 ? <span className="text-white/50"> · </span> : null}
                <span className={SCOPE_STATUS_COLOR[status]}>
                  {count} {t(SERVICE_STATUS_LABEL_KEYS[status])}
                </span>
              </span>
            ))}
          </p>
        ) : (
          <p className="text-xs text-white/70">
            {services.length} {t("services")}
          </p>
        )}
      </div>
    </div>
  )
}

