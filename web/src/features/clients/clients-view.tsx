import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import {
  ChevronRightIcon,
  CircleDashed,
  FileJsonIcon,
  FolderIcon,
  LayoutGridIcon,
  ListIcon,
  PencilIcon,
  PlusIcon,
  RefreshCwIcon,
  StoreIcon,
  TerminalIcon,
  UploadIcon,
} from "lucide-react"
import { useQueryClient } from "@tanstack/react-query"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"

import { clientIconUrls } from "@/assets/icons/clients"
import { CollapsibleSearchBox } from "@/components/shared/collapsible-search-box"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  ClientPluginRowActions,
  useClientPluginMarketQuery,
  useClientPluginsQuery,
  usePluginAction,
  usePluginUpdate,
} from "@/features/clients/client-plugins-panel"
import {
  useClientsQuery,
  useClientToggle,
  useClientVersionQuery,
  useWiredClientsQuery,
} from "@/features/clients/queries"
import { useI18n } from "@/lib/i18n-context"
import type { ClientInventory, ExposeConfig, WiredEntry, WireOptions, WireResult } from "@/lib/api"
import type { ClientPlugin, MarketPlugin } from "@/lib/api/plugins"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

type ClientTab = "active" | "inactive" | "all"
type ClientsViewMode = "cards" | "list"

const VIEW_MODE_STORAGE_KEY = "mcpstore.clients-view-mode"

function readViewMode(): ClientsViewMode {
  try {
    const value = localStorage.getItem(VIEW_MODE_STORAGE_KEY)
    return value === "list" ? "list" : "cards"
  } catch {
    return "cards"
  }
}

function writeViewMode(mode: ClientsViewMode) {
  try {
    localStorage.setItem(VIEW_MODE_STORAGE_KEY, mode)
  } catch {
    // Ignore storage failures.
  }
}

export const EXPOSE_KEYS: (keyof ExposeConfig)[] = [
  "tool",
  "prompt",
  "resource",
  "openapi",
  "service",
  "cache",
  "search",
  "session_state",
]

const DEFAULT_EXPOSE: ExposeConfig = {
  session_state: false,
  tool: false,
  prompt: false,
  resource: false,
  openapi: false,
  service: false,
  cache: false,
  search: false,
}

/** Form fields for scope + the 8 expose toggles, shared by the wire dialog and the gateway page edit dialog. */
export function WireOptionsFields({
  dedicatedScope,
  dedicatedScopeLabel,
  expose,
  onDedicatedScopeChange,
  onExposeChange,
}: {
  dedicatedScope: boolean
  dedicatedScopeLabel: string
  expose: ExposeConfig
  onDedicatedScopeChange: (dedicated: boolean) => void
  onExposeChange: (expose: ExposeConfig) => void
}) {
  const { t } = useI18n()
  return (
    <>
      <div className="flex items-center justify-between gap-3">
        <div className="flex flex-col gap-0.5">
          <Label htmlFor="wire-scope">{t("clientsScope")}</Label>
          <span className="font-mono text-xs text-muted-foreground">
            {dedicatedScope ? dedicatedScopeLabel : t("gatewayScopeStore")}
          </span>
        </div>
        <Switch id="wire-scope" checked={dedicatedScope} onCheckedChange={onDedicatedScopeChange} />
      </div>
      <div className="flex flex-col gap-2">
        <Label>{t("clientsExpose")}</Label>
        <div className="grid grid-cols-2 gap-x-4 gap-y-2">
          {EXPOSE_KEYS.map((key) => (
            <div key={key} className="flex cursor-pointer items-center gap-2 text-sm">
              <Switch
                checked={expose[key]}
                onCheckedChange={(on) => onExposeChange({ ...expose, [key]: on })}
              />
              <span className="font-mono text-xs">{key}</span>
            </div>
          ))}
        </div>
      </div>
    </>
  )
}

export function ClientsView() {
  const { t } = useI18n()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const { data, isPending, isError, error, isFetching } = useClientsQuery()
  const wiredQuery = useWiredClientsQuery()
  const { toggle, busy } = useClientToggle()
  const pluginAction = usePluginAction()
  const pluginUpdate = usePluginUpdate()
  const pluginsQuery = useClientPluginsQuery()
  const marketQuery = useClientPluginMarketQuery()
  const [tab, setTab] = useState<ClientTab>("active")
  const [query, setQuery] = useState("")
  const [viewMode, setViewMode] = useState<ClientsViewMode>(readViewMode)
  const [cardClientId, setCardClientId] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)
  const [wireDialog, setWireDialog] = useState<WireDialogState | null>(null)
  const [wireConflict, setWireConflict] = useState<WireConflictState | null>(null)

  const clients = data ?? []
  const pluginBusy = busy || pluginAction.isPending || pluginUpdate.isPending
  const refreshing =
    isFetching || wiredQuery.isFetching || pluginsQuery.isFetching || marketQuery.isFetching
  const installedById = Object.fromEntries(
    (pluginsQuery.data?.plugins ?? []).map((plugin) => [plugin.id, plugin]),
  ) as Record<string, ClientPlugin>
  const marketById = Object.fromEntries(
    (marketQuery.data?.plugins ?? []).map((entry) => [entry.id, entry]),
  ) as Record<string, MarketPlugin>
  const wiredById = Object.fromEntries(
    (wiredQuery.data ?? []).map((entry) => [entry.client_id, entry]),
  ) as Record<string, WiredEntry>
  const cardClient = cardClientId
    ? (clients.find((client) => client.id === cardClientId) ?? null)
    : null

  function handleViewModeChange(mode: ClientsViewMode) {
    setViewMode(mode)
    writeViewMode(mode)
    if (mode === "list") setCardClientId(null)
  }

  const tabs = useMemo(
    () => [
      { id: "active" as const, label: t("clientsTabActive") },
      { id: "inactive" as const, label: t("clientsTabInactive") },
      { id: "all" as const, label: t("clientsTabAll") },
    ],
    [t],
  )

  // Agents only lists installed adapters (active + inactive). not_installed lives in Plugins.
  const installedClients = useMemo(
    () =>
      clients.filter(
        (client) => client.wired || client.detected_by != null || Boolean(installedById[client.id]),
      ),
    [clients, installedById],
  )

  // Sticky membership for active/inactive: wire/unwire updates status in place,
  // but cards stay until the user re-enters the tab (or remounts the page).
  const membershipRef = useRef<{ tab: ClientTab; ids: Set<string> } | null>(null)
  const membershipTabRef = useRef<ClientTab>(tab)
  const [membershipVersion, setMembershipVersion] = useState(0)

  useEffect(() => {
    const tabChanged = membershipTabRef.current !== tab
    membershipTabRef.current = tab

    if (tab === "all") {
      membershipRef.current = null
      if (tabChanged) setMembershipVersion((value) => value + 1)
      return
    }

    const existing = membershipRef.current
    const sameTab = existing?.tab === tab
    // Keep sticky ids while staying on the same tab (ignore wired flips).
    // Re-seed only when entering the tab, or when data first arrives into an empty snapshot.
    if (sameTab && !tabChanged && existing.ids.size > 0) return

    membershipRef.current = {
      tab,
      ids: new Set(
        installedClients
          .filter((client) => (tab === "active" ? client.wired : !client.wired))
          .map((client) => client.id),
      ),
    }
    setMembershipVersion((value) => value + 1)
  }, [tab, installedClients])

  const visibleClients = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    const membership = membershipRef.current
    return installedClients.filter((client) => {
      if (tab === "active" || tab === "inactive") {
        if (membership?.tab === tab) {
          if (!membership.ids.has(client.id)) return false
        } else if (tab === "active" ? !client.wired : client.wired) {
          return false
        }
      }
      if (!normalized) return true
      return (
        client.id.toLowerCase().includes(normalized) ||
        client.label.toLowerCase().includes(normalized)
      )
    })
  }, [installedClients, query, tab, membershipVersion])

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
    void queryClient.invalidateQueries({ queryKey: queryKeys.wiredClients })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPlugins })
    void queryClient.invalidateQueries({ queryKey: queryKeys.clientPluginMarket })
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("clientsWorkspaceTitle")}</h1>}
        subtitle={t("clientsWorkspaceDescription", { count: visibleClients.length })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0">
        <Tabs
          value={tab}
          onValueChange={(value) => setTab(value as ClientTab)}
          className="flex min-h-0 flex-1 flex-col overflow-hidden"
        >
          <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              {tabs.map((item) => (
                <TabsTrigger key={item.id} value={item.id} title={item.label} className="flex-none">
                  {item.label}
                </TabsTrigger>
              ))}
            </TabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              <CollapsibleSearchBox
                inline
                id="client-list-search"
                placeholder={t("searchClients")}
                value={query}
                onChange={setQuery}
              />
              <Button size="sm" variant="outline" onClick={refresh} disabled={refreshing}>
                <RefreshCwIcon data-icon="inline-start" />
                {t("refresh")}
              </Button>
              <Button size="sm" onClick={() => setAddOpen(true)}>
                <PlusIcon data-icon="inline-start" />
                {t("clientsAddCardLabel")}
              </Button>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  aria-label={t("servicesViewCards")}
                  aria-pressed={viewMode === "cards"}
                  size="icon-sm"
                  title={t("servicesViewCards")}
                  variant={viewMode === "cards" ? "secondary" : "outline"}
                  onClick={() => handleViewModeChange("cards")}
                >
                  <LayoutGridIcon aria-hidden="true" />
                </Button>
                <Button
                  aria-label={t("servicesViewList")}
                  aria-pressed={viewMode === "list"}
                  size="icon-sm"
                  title={t("servicesViewList")}
                  variant={viewMode === "list" ? "secondary" : "outline"}
                  onClick={() => handleViewModeChange("list")}
                >
                  <ListIcon aria-hidden="true" />
                </Button>
              </div>
            </div>
          </div>

          {tabs.map((item) => (
            <TabsContent key={item.id} value={item.id} className={tabContentClassName}>
              {tab !== item.id ? null : isError ? (
                <div className="px-4 py-2">
                  <PageError
                    title={t("clientsLoadFailed")}
                    message={error instanceof Error ? error.message : String(error)}
                    onRefresh={refresh}
                  />
                </div>
              ) : isPending ? (
                <div className="px-4 py-2">
                  <PageSkeleton />
                </div>
              ) : viewMode === "list" ? (
                <ScrollPane
                  className="min-h-0 flex-1"
                  innerClassName="flex flex-col gap-2 px-0 pb-2"
                >
                  {visibleClients.length ? (
                    <div className="divide-y divide-border/50">
                      {visibleClients.map((client) => (
                        <ClientRow
                          key={client.id}
                          client={client}
                          busy={pluginBusy}
                          installed={installedById[client.id]}
                          marketEntry={marketById[client.id]}
                          onWire={() => setWireDialog({ mode: "wire", client })}
                          onEditWire={() => {
                            const entry = wiredById[client.id]
                            if (entry) setWireDialog({ mode: "edit", client, entry })
                          }}
                          onUnwire={() => void toggle(client, true)}
                          onInstall={(id) => pluginAction.mutate({ action: "install", id })}
                          onUninstall={(id) => pluginAction.mutate({ action: "uninstall", id })}
                          onUpdate={(id) => pluginUpdate.mutate(id)}
                        />
                      ))}
                    </div>
                  ) : (
                    <div className="px-4 py-2">
                      <PageEmpty
                        title={t("noClients")}
                        description={t("noClientsDescription")}
                        onRefresh={refresh}
                      />
                    </div>
                  )}
                </ScrollPane>
              ) : (
                <>
                  <ScrollPane
                    className="min-h-0 flex-1"
                    innerClassName="px-4 py-2 grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4"
                  >
                    {visibleClients.map((client) => (
                      <ClientListCard
                        key={client.id}
                        client={client}
                        onSelect={() => setCardClientId(client.id)}
                      />
                    ))}
                    <AddAgentCard onSelect={() => setAddOpen(true)} />
                  </ScrollPane>
                  {cardClient ? (
                    <ClientDetailDialog
                      client={cardClient}
                      open
                      onOpenChange={(open) => {
                        if (!open) setCardClientId(null)
                      }}
                      actions={
                        <ClientCardActions
                          busy={pluginBusy}
                          client={cardClient}
                          installed={installedById[cardClient.id]}
                          marketEntry={marketById[cardClient.id]}
                          onWire={() => {
                            setCardClientId(null)
                            setWireDialog({ mode: "wire", client: cardClient })
                          }}
                          onEditWire={() => {
                            const entry = wiredById[cardClient.id]
                            if (!entry) return
                            setCardClientId(null)
                            setWireDialog({ mode: "edit", client: cardClient, entry })
                          }}
                          onUnwire={() => void toggle(cardClient, true)}
                          onInstall={(id) => pluginAction.mutate({ action: "install", id })}
                          onUninstall={(id) => pluginAction.mutate({ action: "uninstall", id })}
                          onUpdate={(id) => pluginUpdate.mutate(id)}
                        />
                      }
                    />
                  ) : null}
                </>
              )}
            </TabsContent>
          ))}
        </Tabs>
      </PanelCard>

      <AddAgentDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onBrowseMarket={() => {
          setAddOpen(false)
          navigate("/plugins?tab=agents")
        }}
      />

      <WireDialog
        state={wireDialog}
        busy={busy}
        onClose={() => setWireDialog(null)}
        onConfirm={async (client, options) => {
          const editing = wireDialog?.mode === "edit"
          setWireDialog(null)
          const result = await toggle(client, false, options)
          if (!editing && result?.outcome === "conflict") {
            setWireConflict({ client, options, result })
          }
        }}
      />

      <WireConflictDialog
        state={wireConflict}
        busy={busy}
        onClose={() => setWireConflict(null)}
        onReplace={async (client, options) => {
          setWireConflict(null)
          await toggle(client, false, options, true)
        }}
      />
    </>
  )
}

type WireDialogState =
  | { mode: "wire"; client: ClientInventory }
  | { mode: "edit"; client: ClientInventory; entry: WiredEntry }

/** Wire conflict: the target file already contains an mcpstore entry with different content; waiting for the user to confirm replacement. */
type WireConflictState = {
  client: ClientInventory
  options: WireOptions
  result: Extract<WireResult, { outcome: "conflict" }>
}

/** Wire / edit an existing wiring: scope + expose toggles; edit mode prefills effective values from /gateway/wired. */
function WireDialog({
  state,
  busy,
  onClose,
  onConfirm,
}: {
  state: WireDialogState | null
  busy: boolean
  onClose: () => void
  onConfirm: (client: ClientInventory, options: WireOptions) => void
}) {
  const { t } = useI18n()
  const [dedicatedScope, setDedicatedScope] = useState(true)
  const [expose, setExpose] = useState<ExposeConfig>(DEFAULT_EXPOSE)
  const client = state?.client ?? null
  const editing = state?.mode === "edit"

  useEffect(() => {
    if (!state) return
    if (state.mode === "edit") {
      setDedicatedScope(state.entry.scope.type === "agent")
      setExpose(state.entry.expose)
      return
    }
    setDedicatedScope(true)
    setExpose(DEFAULT_EXPOSE)
  }, [state])

  if (!client || !state) return null
  const agentId =
    state.mode === "edit" && state.entry.scope.type === "agent"
      ? state.entry.scope.agent_id
      : client.id
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {editing
              ? t("gatewayEditWireTitle", { client: client.label })
              : t("clientsWireDialogTitle", { client: client.label })}
          </DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <WireOptionsFields
            dedicatedScope={dedicatedScope}
            dedicatedScopeLabel={`Agent:${agentId}`}
            expose={expose}
            onDedicatedScopeChange={setDedicatedScope}
            onExposeChange={setExpose}
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button
            disabled={busy}
            onClick={() =>
              onConfirm(client, {
                scope: dedicatedScope ? "agent" : "store",
                agentId: dedicatedScope ? agentId : undefined,
                expose,
              })
            }
          >
            {editing ? t("save") : t("clientsWire")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** Conflict confirmation: shows the existing entry and the values to be written side by side; on confirm re-sends with force to replace. */
function WireConflictDialog({
  state,
  busy,
  onClose,
  onReplace,
}: {
  state: WireConflictState | null
  busy: boolean
  onClose: () => void
  onReplace: (client: ClientInventory, options: WireOptions) => void
}) {
  const { t } = useI18n()
  if (!state) return null
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("clientsWireConflictTitle", { client: state.client.label })}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          {t("clientsWireConflictMessage", { path: state.result.path })}
        </p>
        <div className="grid grid-cols-2 gap-3">
          <div className="min-w-0">
            <Label>{t("clientsWireConflictCurrent")}</Label>
            <pre className="mt-1 max-h-36 overflow-auto rounded-md bg-muted p-2 font-mono text-xs break-all whitespace-pre-wrap">
              {JSON.stringify(state.result.current, null, 2)}
            </pre>
          </div>
          <div className="min-w-0">
            <Label>{t("clientsWireConflictExpected")}</Label>
            <pre className="mt-1 max-h-36 overflow-auto rounded-md bg-muted p-2 font-mono text-xs break-all whitespace-pre-wrap">
              {JSON.stringify(state.result.expected, null, 2)}
            </pre>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button disabled={busy} onClick={() => onReplace(state.client, state.options)}>
            {t("clientsWireConflictReplace")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function clientStatusText(
  client: ClientInventory,
  t: (key: string, vars?: Record<string, string | number | null | undefined>) => string,
) {
  return client.wired ? t("clientsStatusActive") : t("clientsStatusInactive")
}

function StatusDot({ active }: { active: boolean }) {
  return (
    <span
      className={cn(
        "size-1.5 shrink-0 rounded-full",
        active ? "bg-emerald-500" : "bg-muted-foreground/40",
      )}
    />
  )
}

/** Plugin install/update actions + edit-wire pencil + wire toggle, shared by the list row and the card detail dialog. */
function ClientWireActions({
  busy,
  client,
  installed,
  marketEntry,
  onInstall,
  onUninstall,
  onUpdate,
  onWire,
  onEditWire,
  onUnwire,
  showWireLabel,
}: {
  busy: boolean
  client: ClientInventory
  installed: ClientPlugin | undefined
  marketEntry: MarketPlugin | undefined
  onInstall: (id: string) => void
  onUninstall: (id: string) => void
  onUpdate: (id: string) => void
  onWire: () => void
  onEditWire: () => void
  onUnwire: () => void
  showWireLabel?: boolean
}) {
  const { t } = useI18n()
  const wireSwitch = (
    <Switch
      checked={client.wired}
      disabled={busy}
      aria-label={client.wired ? t("clientsUnwire") : t("clientsWire")}
      onCheckedChange={(checked) => {
        if (checked) onWire()
        else onUnwire()
      }}
    />
  )
  return (
    <>
      <ClientPluginRowActions
        clientId={client.id}
        busy={busy}
        installed={installed}
        marketEntry={marketEntry}
        onInstall={onInstall}
        onUninstall={onUninstall}
        onUpdate={onUpdate}
      />
      {client.wired ? (
        <Button
          size="icon-sm"
          variant="ghost"
          disabled={busy}
          aria-label={t("gatewayEditWire")}
          onClick={onEditWire}
        >
          <PencilIcon className="size-3.5" />
        </Button>
      ) : null}
      {showWireLabel ? (
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">
            {client.wired ? t("clientsUnwire") : t("clientsWire")}
          </span>
          {wireSwitch}
        </div>
      ) : (
        wireSwitch
      )}
    </>
  )
}

function ClientRow({
  busy,
  client,
  installed,
  marketEntry,
  onWire,
  onEditWire,
  onUnwire,
  onInstall,
  onUninstall,
  onUpdate,
}: {
  busy: boolean
  client: ClientInventory
  installed: ClientPlugin | undefined
  marketEntry: MarketPlugin | undefined
  onWire: () => void
  onEditWire: () => void
  onUnwire: () => void
  onInstall: (id: string) => void
  onUninstall: (id: string) => void
  onUpdate: (id: string) => void
}) {
  const { t } = useI18n()
  const [detailOpen, setDetailOpen] = useState(false)
  const detected = client.detected_by != null
  const mcpCount = client.mcp_servers?.length ?? 0
  const showMcp = client.mcp_servers != null
  const versionQuery = useClientVersionQuery(client.id, detected)
  const version = versionQuery.data ?? client.version ?? null
  const statusText = clientStatusText(client, t)

  return (
    <>
      <div className="flex items-center gap-3 px-4 py-3">
        <ClientLogo clientId={client.id} label={client.label} />

        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-baseline gap-2">
            <span className="truncate text-sm font-medium">{client.id}</span>
            {version ? (
              <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{version}</span>
            ) : null}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <StatusDot active={client.wired} />
            <span className="truncate">{statusText}</span>
          </div>
        </div>

        {showMcp ? (
          <button
            type="button"
            className="inline-flex shrink-0 items-center gap-0.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label={t("clientsDetailTitle", { client: client.label })}
            onClick={() => setDetailOpen(true)}
          >
            <span>{t("clientsMcpShort")}</span>
            <span className="tabular-nums">{mcpCount}</span>
            <ChevronRightIcon className="size-3.5" />
          </button>
        ) : (
          <button
            type="button"
            className="inline-flex shrink-0 items-center gap-0.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label={t("clientsDetailTitle", { client: client.label })}
            onClick={() => setDetailOpen(true)}
          >
            <ChevronRightIcon className="size-3.5" />
          </button>
        )}

        <ClientWireActions
          busy={busy}
          client={client}
          installed={installed}
          marketEntry={marketEntry}
          onInstall={onInstall}
          onUninstall={onUninstall}
          onUpdate={onUpdate}
          onWire={onWire}
          onEditWire={onEditWire}
          onUnwire={onUnwire}
        />
      </div>

      <ClientDetailDialog
        client={client}
        version={version}
        open={detailOpen}
        onOpenChange={setDetailOpen}
      />
    </>
  )
}

function ClientListCard({ client, onSelect }: { client: ClientInventory; onSelect: () => void }) {
  const { t } = useI18n()
  const mcpCount = client.mcp_servers?.length ?? 0
  const statusText = clientStatusText(client, t)
  const path = client.config_path?.trim()

  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left transition-colors hover:border-border hover:bg-muted/40"
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <div className="flex min-w-0 items-start gap-2">
        <ClientLogo
          clientId={client.id}
          label={client.label}
          className="size-8 rounded-md border border-border/50 bg-muted/60"
          imageClassName="size-5"
        />
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 text-sm font-semibold leading-snug">
            {client.label || client.id}
          </span>
          <span className="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <StatusDot active={client.wired} />
            <span className="truncate">{statusText}</span>
          </span>
        </span>
      </div>

      {path ? (
        <p className="mt-3 min-w-0 w-full truncate text-sm text-muted-foreground">{path}</p>
      ) : null}

      {client.mcp_servers != null ? (
        <p className="mt-3 text-xs text-muted-foreground">
          {t("clientsMcpShort")} {mcpCount}
        </p>
      ) : null}
    </div>
  )
}

function AddAgentCard({ onSelect }: { onSelect: () => void }) {
  const { t } = useI18n()
  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
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
      <span className="text-sm font-medium">{t("clientsAddCardLabel")}</span>
    </div>
  )
}

function AddAgentDialog({
  open,
  onOpenChange,
  onBrowseMarket,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onBrowseMarket: () => void
}) {
  const { t } = useI18n()
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("clientsAddTitle")}</DialogTitle>
          <DialogDescription>{t("clientsAddDescription")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <button
            type="button"
            className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 px-4 py-8 text-center transition-colors hover:border-border hover:bg-muted/40"
            onClick={() => toast.message(t("clientsAddImportSoon"))}
          >
            <UploadIcon className="size-6 text-muted-foreground" />
            <span className="text-sm font-medium">{t("clientsAddImport")}</span>
            <span className="text-xs text-muted-foreground">{t("clientsAddImportHint")}</span>
          </button>
          <Button variant="outline" onClick={onBrowseMarket}>
            <StoreIcon data-icon="inline-start" />
            {t("clientsAddMarket")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function ClientCardActions({
  busy,
  client,
  installed,
  marketEntry,
  onWire,
  onEditWire,
  onUnwire,
  onInstall,
  onUninstall,
  onUpdate,
}: {
  busy: boolean
  client: ClientInventory
  installed: ClientPlugin | undefined
  marketEntry: MarketPlugin | undefined
  onWire: () => void
  onEditWire: () => void
  onUnwire: () => void
  onInstall: (id: string) => void
  onUninstall: (id: string) => void
  onUpdate: (id: string) => void
}) {
  return (
    <div className="flex w-full flex-wrap items-center justify-end gap-2">
      <ClientWireActions
        busy={busy}
        client={client}
        installed={installed}
        marketEntry={marketEntry}
        onInstall={onInstall}
        onUninstall={onUninstall}
        onUpdate={onUpdate}
        onWire={onWire}
        onEditWire={onEditWire}
        onUnwire={onUnwire}
        showWireLabel
      />
    </div>
  )
}

function ClientLogo({
  clientId,
  label,
  className,
  imageClassName,
}: {
  clientId: string
  label: string
  className?: string
  imageClassName?: string
}) {
  const iconUrl = clientIconUrls[clientId]
  return (
    <span className={cn("flex size-8 shrink-0 items-center justify-center", className)}>
      {iconUrl ? (
        <img
          src={iconUrl}
          alt=""
          className={cn("size-7 object-contain", imageClassName)}
          aria-hidden
        />
      ) : (
        <span
          aria-hidden
          className={cn(
            "flex size-7 items-center justify-center rounded-md bg-muted text-xs font-medium uppercase",
            imageClassName,
          )}
        >
          {clientId.slice(0, 2)}
        </span>
      )}
      <span className="sr-only">{label}</span>
    </span>
  )
}

function ClientDetailDialog({
  actions,
  client,
  version: versionProp,
  open,
  onOpenChange,
}: {
  actions?: ReactNode
  client: ClientInventory
  version?: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const detected = client.detected_by != null
  const versionQuery = useClientVersionQuery(client.id, open && detected && versionProp == null)
  const version = versionProp ?? versionQuery.data ?? client.version ?? null
  const statusText = clientStatusText(client, t)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="sr-only">
            {t("clientsDetailTitle", { client: client.label })}
          </DialogTitle>
          <div className="flex items-center gap-3 pr-6">
            <ClientLogo
              clientId={client.id}
              label={client.label}
              className="size-12 rounded-xl border border-border/50 bg-muted/60"
              imageClassName="size-8"
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-base font-semibold">{client.label || client.id}</p>
              <p className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                <StatusDot active={client.wired} />
                <span className="truncate">{statusText}</span>
                {version ? (
                  <span className="shrink-0 font-mono tabular-nums text-muted-foreground">
                    · {version}
                  </span>
                ) : null}
              </p>
            </div>
          </div>
        </DialogHeader>
        <dl className="flex flex-col gap-3 text-sm">
          <DetailRow label={t("clientsConfigPath")}>
            <span className="font-mono text-xs break-all">
              {client.config_path}
              {client.config_exists ? "" : ` (${t("clientsConfigMissing")})`}
            </span>
          </DetailRow>
          {client.wired_url ? (
            <DetailRow label={t("clientsWiredUrl")}>
              <span className="font-mono text-xs break-all">{client.wired_url}</span>
            </DetailRow>
          ) : null}
          <DetailRow label={t("clientsDetected")}>
            <DetectedDetail client={client} />
          </DetailRow>
          {client.mcp_servers != null ? (
            <DetailRow label={t("clientsServers")}>
              {client.mcp_servers.length > 0 ? (
                <ul className="flex flex-col gap-1 font-mono text-xs">
                  {client.mcp_servers.map((name) => (
                    <li key={name}>{name}</li>
                  ))}
                </ul>
              ) : (
                <span className="text-muted-foreground">—</span>
              )}
            </DetailRow>
          ) : null}
          {client.unsupported_fields.length > 0 ? (
            <DetailRow label={t("clientsUnsupported")}>
              <ul className="flex flex-col gap-1 font-mono text-xs text-amber-600 dark:text-amber-400">
                {client.unsupported_fields.map((field) => (
                  <li key={field}>{field}</li>
                ))}
              </ul>
            </DetailRow>
          ) : null}
        </dl>
        {actions ? <DialogFooter>{actions}</DialogFooter> : null}
      </DialogContent>
    </Dialog>
  )
}

function DetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd>{children}</dd>
    </div>
  )
}

function DetectedDetail({ client }: { client: ClientInventory }) {
  const { t } = useI18n()
  const icon =
    client.detected_by === "config_file" ? (
      <FileJsonIcon className="size-3.5 shrink-0" />
    ) : client.detected_by === "config_dir" ? (
      <FolderIcon className="size-3.5 shrink-0" />
    ) : client.detected_by === "command" ? (
      <TerminalIcon className="size-3.5 shrink-0" />
    ) : (
      <CircleDashed className="size-3.5 shrink-0" />
    )

  return (
    <span className="inline-flex items-center gap-1.5 text-muted-foreground">
      {icon}
      {client.detected_by ? t("clientsDetected") : t("clientsNotDetected")}
    </span>
  )
}
