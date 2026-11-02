import { useEffect, useMemo, useState, type ReactNode } from "react"
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
import type {
  ClientInventory,
  ExposeConfig,
  WiredEntry,
  WireOptions,
  WireResult,
} from "@/lib/api"
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

/** 作用域 + 8 个暴露开关的表单体，供接线弹窗与网关页编辑弹窗共用。 */
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
            <label key={key} className="flex cursor-pointer items-center gap-2 text-sm">
              <Switch
                checked={expose[key]}
                onCheckedChange={(on) => onExposeChange({ ...expose, [key]: on })}
              />
              <span className="font-mono text-xs">{key}</span>
            </label>
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
  const refreshing = isFetching || wiredQuery.isFetching || pluginsQuery.isFetching || marketQuery.isFetching
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
        (client) =>
          client.wired || client.detected_by != null || Boolean(installedById[client.id]),
      ),
    [clients, installedById],
  )

  const visibleClients = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return installedClients.filter((client) => {
      if (tab === "active" && !client.wired) return false
      if (tab === "inactive" && client.wired) return false
      if (!normalized) return true
      return (
        client.id.toLowerCase().includes(normalized) ||
        client.label.toLowerCase().includes(normalized)
      )
    })
  }, [installedClients, query, tab])

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
        <Tabs value={tab} onValueChange={(value) => setTab(value as ClientTab)} className="flex min-h-0 flex-1 flex-col overflow-hidden">
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
                <ScrollPane className="min-h-0 flex-1" innerClassName="flex flex-col gap-2 px-0 pb-2">
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
                      <PageEmpty title={t("noClients")} description={t("noClientsDescription")} onRefresh={refresh} />
                    </div>
                  )}
                </ScrollPane>
              ) : (
                <>
                  <ScrollPane
                    className="min-h-0 flex-1"
                    innerClassName="px-4 py-2 grid auto-rows-fr grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-3"
                  >
                    {visibleClients.map((client) => (
                      <ClientListCard
                        key={client.id}
                        busy={pluginBusy}
                        client={client}
                        installed={installedById[client.id]}
                        marketEntry={marketById[client.id]}
                        onSelect={() => setCardClientId(client.id)}
                        onWire={() => setWireDialog({ mode: "wire", client })}
                        onUnwire={() => void toggle(client, true)}
                        onInstall={(id) => pluginAction.mutate({ action: "install", id })}
                        onUninstall={(id) => pluginAction.mutate({ action: "uninstall", id })}
                        onUpdate={(id) => pluginUpdate.mutate(id)}
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

/** 接线冲突：目标文件里已有一条内容不一致的 mcpstore 条目，等用户确认替换。 */
type WireConflictState = {
  client: ClientInventory
  options: WireOptions
  result: Extract<WireResult, { outcome: "conflict" }>
}

/** 接线 / 编辑已接入配置：作用域 + 暴露开关；编辑时预填 /gateway/wired 生效值。 */
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
    <Dialog open onOpenChange={(open) => { if (!open) onClose() }}>
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
          <Button variant="outline" onClick={onClose}>{t("cancel")}</Button>
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

/** 冲突确认：并排展示现有条目与将写入的值，用户确认后带 force 重发替换。 */
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
    <Dialog open onOpenChange={(open) => { if (!open) onClose() }}>
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
          <Button variant="outline" onClick={onClose}>{t("cancel")}</Button>
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
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                client.wired ? "bg-emerald-500" : "bg-muted-foreground/40",
              )}
            />
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

        <Switch
          checked={client.wired}
          disabled={busy}
          aria-label={client.wired ? t("clientsUnwire") : t("clientsWire")}
          onCheckedChange={(checked) => {
            if (checked) onWire()
            else onUnwire()
          }}
        />
      </div>

      <ClientDetailDialog client={client} version={version} open={detailOpen} onOpenChange={setDetailOpen} />
    </>
  )
}

function ClientListCard({
  busy,
  client,
  installed,
  marketEntry,
  onSelect,
  onWire,
  onUnwire,
  onInstall,
  onUninstall,
  onUpdate,
}: {
  busy: boolean
  client: ClientInventory
  installed: ClientPlugin | undefined
  marketEntry: MarketPlugin | undefined
  onSelect: () => void
  onWire: () => void
  onUnwire: () => void
  onInstall: (id: string) => void
  onUninstall: (id: string) => void
  onUpdate: (id: string) => void
}) {
  const { t } = useI18n()
  const detected = client.detected_by != null
  const versionQuery = useClientVersionQuery(client.id, detected)
  const version = versionQuery.data ?? client.version ?? null
  const statusText = clientStatusText(client, t)

  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex h-full min-h-36 cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left shadow-xs transition-colors hover:border-border hover:bg-muted/40"
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <div className="flex min-w-0 items-center gap-3">
        <ClientLogo
          clientId={client.id}
          label={client.label}
          className="size-10 rounded-lg border border-border/50 bg-muted/60"
          imageClassName="size-7"
        />
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 text-sm font-semibold leading-snug">{client.label || client.id}</span>
          <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                client.wired ? "bg-emerald-500" : "bg-muted-foreground/40",
              )}
            />
            <span className="truncate">{statusText}</span>
          </span>
        </span>
        <div
          className="shrink-0"
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
        >
          <Switch
            checked={client.wired}
            disabled={busy}
            aria-label={client.wired ? t("clientsUnwire") : t("clientsWire")}
            onCheckedChange={(checked) => {
              if (checked) onWire()
              else onUnwire()
            }}
          />
        </div>
      </div>

      <p className="mt-3 line-clamp-2 flex-1 text-xs leading-relaxed text-muted-foreground">
        {client.config_path || client.id}
      </p>

      <div className="mt-3 flex min-w-0 items-center justify-between gap-2">
        <span className="shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
          {version || "—"}
        </span>
        <div
          className="shrink-0"
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
        >
          <ClientPluginRowActions
            clientId={client.id}
            busy={busy}
            installed={installed}
            marketEntry={marketEntry}
            onInstall={onInstall}
            onUninstall={onUninstall}
            onUpdate={onUpdate}
          />
        </div>
      </div>
    </div>
  )
}

function AddAgentCard({ onSelect }: { onSelect: () => void }) {
  const { t } = useI18n()
  return (
    <div
      role="button"
      tabIndex={0}
      className="group flex h-full min-h-36 cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onSelect()
        }
      }}
    >
      <span className="flex size-10 items-center justify-center rounded-lg border border-dashed border-border/70">
        <PlusIcon className="size-5" />
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
  const { t } = useI18n()
  return (
    <div className="flex w-full flex-wrap items-center justify-end gap-2">
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
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">
          {client.wired ? t("clientsUnwire") : t("clientsWire")}
        </span>
        <Switch
          checked={client.wired}
          disabled={busy}
          aria-label={client.wired ? t("clientsUnwire") : t("clientsWire")}
          onCheckedChange={(checked) => {
            if (checked) onWire()
            else onUnwire()
          }}
        />
      </div>
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("clientsDetailTitle", { client: client.label })}</DialogTitle>
        </DialogHeader>
        <dl className="flex flex-col gap-3 text-sm">
          {version ? (
            <DetailRow label={t("version")}>
              <span className="font-mono text-xs tabular-nums">{version}</span>
            </DetailRow>
          ) : null}
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
