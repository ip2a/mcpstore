import { useEffect, useState, type ReactNode } from "react"
import { SettingsIcon, WaypointsIcon } from "lucide-react"
import { toast } from "sonner"

import { clientIconUrls } from "@/assets/icons/clients"
import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { MarkIcon } from "@/components/shared/mark-icon"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_PANEL_CLASSNAME,
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import {
  EXPOSE_KEYS,
} from "@/features/clients/clients-view"
import {
  useClientsQuery,
  useGatewayProjectionQuery,
  useGatewayStatusQuery,
  useWiredClientsQuery,
} from "@/features/clients/queries"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { FrameStreamCard } from "@/features/traces/frame-stream-card"
import { useI18n } from "@/lib/i18n-context"
import {
  getGatewayRequests,
  startMcpHub,
  stopMcpHub,
  type GatewayProjectionIssue,
  type GatewayRequestRecord,
  type GatewayStatus,
  type McpHubOptions,
  type WiredEntry,
} from "@/lib/api"
import { cn } from "@/lib/utils"

const DEFAULT_LAUNCH_OPTIONS: McpHubOptions = {
  transport: "streamable-http",
  scope: "store",
  host: "127.0.0.1",
  path: "/mcp",
}

type GatewayPanelTab = "calls" | "projection" | "table" | "graph"

export function GatewayView() {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const clients = useClientsQuery()
  const wired = useWiredClientsQuery()
  const records = useGatewayRequestFlow()
  const projection = useGatewayProjectionQuery(status.data?.running === true)
  const meta = useSettingsMetaQuery()
  const updateSettings = useUpdateSettingsMutation()
  const [busy, setBusy] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [launchOptions, setLaunchOptions] = useState<McpHubOptions>(DEFAULT_LAUNCH_OPTIONS)
  const [panelTab, setPanelTab] = useState<GatewayPanelTab>("calls")

  const wiredLabels = (clients.data ?? [])
    .filter((client) => client.wired)
    .map((client) => client.label)
  const labelById = new Map((clients.data ?? []).map((client) => [client.id, client.label]))

  const defaultPort = meta.data?.settings?.gateway?.port
  useEffect(() => {
    if (defaultPort) {
      setLaunchOptions((prev) => (prev.port ? prev : { ...prev, port: defaultPort }))
    }
  }, [defaultPort])

  async function handleStart(options: McpHubOptions, persistPort: boolean) {
    setBusy(true)
    try {
      await startMcpHub(options)
      if (persistPort && options.port) {
        await updateSettings.mutateAsync({ gateway: { port: options.port } })
        await meta.refetch()
      }
      await status.refetch()
      toast.success(t("gatewayStarted"))
      setSettingsOpen(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleStop() {
    setBusy(true)
    try {
      await stopMcpHub()
      await status.refetch()
      toast.success(t("gatewayStopped"))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  const tabContentClassName = "mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("gatewayWorkspaceTitle")}</h1>}
        subtitle={t("gatewayWorkspaceDescription", { count: wiredLabels.length })}
      />

      <LaunchSettingsDialog
        busy={busy}
        onOpenChange={setSettingsOpen}
        onStart={handleStart}
        open={settingsOpen}
        options={launchOptions}
        setOptions={setLaunchOptions}
      />

      <PanelCard className={WORKSPACE_PANEL_CLASSNAME}>
        <Tabs
          value={panelTab}
          onValueChange={(value) => setPanelTab(value as GatewayPanelTab)}
          className={WORKSPACE_TABS_CLASSNAME}
        >
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <WorkspaceTabsList value={panelTab}>
              <TabsTrigger value="calls" className="flex-none">
                {t("gatewayRequestsTitle")}
              </TabsTrigger>
              <TabsTrigger value="projection" className="flex-none">
                {t("gatewayProjectionTitle")}
              </TabsTrigger>
              <TabsTrigger value="table" className="flex-none">
                {t("gatewayFramesTabTable")}
              </TabsTrigger>
              <TabsTrigger value="graph" className="flex-none">
                {t("gatewayFramesTabGraph")}
              </TabsTrigger>
            </WorkspaceTabsList>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              {status.data?.running ? (
                <Badge variant="default">{t("homeRunning")}</Badge>
              ) : (
                <Badge variant="destructive">{t("gatewayStopped")}</Badge>
              )}
              <Button
                disabled={busy}
                size="sm"
                variant="outline"
                onClick={() => setSettingsOpen(true)}
              >
                <SettingsIcon data-icon="inline-start" />
                {t("gatewaySettings")}
              </Button>
              {status.data?.running ? (
                <Button disabled={busy} size="sm" variant="outline" onClick={handleStop}>
                  {t("gatewayStop")}
                </Button>
              ) : (
                <Button
                  disabled={busy}
                  size="sm"
                  onClick={() => handleStart(launchOptions, false)}
                >
                  {t("gatewayStart")}
                </Button>
              )}
            </div>
          </div>

          <TabsContent value="calls" className={tabContentClassName}>
            <ScrollPane
              className="min-h-0 flex-1"
              innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-3 overflow-x-auto")}
            >
              <div className="flex flex-col gap-1 font-mono text-xs text-muted-foreground">
                <span className="truncate">
                  {t("gatewayUrl")}: {status.data?.url ?? "—"}
                </span>
                <span className="truncate">
                  {t("clientsWired")}: {wiredLabels.length > 0 ? wiredLabels.join(", ") : "—"}
                </span>
              </div>
              <CallFlowDiagram
                labelById={labelById}
                latest={records.length > 0 ? records[records.length - 1]! : null}
                status={status.data ?? null}
                wired={wired.data ?? []}
              />
              {records.length === 0 ? (
                <p className="text-sm text-muted-foreground">{t("gatewayRequestsEmpty")}</p>
              ) : (
                <div className="flex flex-col gap-2">
                  {[...records].reverse().map((record) => (
                    <FlowRow key={record.seq} record={record} />
                  ))}
                </div>
              )}
            </ScrollPane>
          </TabsContent>

          <TabsContent value="projection" className={tabContentClassName}>
            <ScrollPane
              className="min-h-0 flex-1"
              innerClassName={cn(WORKSPACE_LIST_INSET_CLASSNAME, "flex flex-col gap-2")}
            >
              {!projection.data ||
              projection.data.mounts.every((mount) => mount.issues.length === 0) ? (
                <p className="text-sm text-muted-foreground">{t("gatewayProjectionEmpty")}</p>
              ) : (
                projection.data.mounts
                  .filter((mount) => mount.issues.length > 0)
                  .map((mount) => (
                    <div key={mount.caller ?? "generic"} className="flex flex-col gap-1">
                      <div className="text-xs font-medium text-muted-foreground">
                        {mount.caller ?? t("gatewayProjectionGenericMount")}
                      </div>
                      {mount.issues.map((issue, index) => (
                        <ProjectionRow key={index} issue={issue} />
                      ))}
                    </div>
                  ))
              )}
            </ScrollPane>
          </TabsContent>

          <TabsContent value="table" className={tabContentClassName}>
            {panelTab === "table" ? (
              <FrameStreamCard face="gateway" embedView="table" fill />
            ) : null}
          </TabsContent>

          <TabsContent value="graph" className={tabContentClassName}>
            {panelTab === "graph" ? (
              <FrameStreamCard face="gateway" embedView="graph" fill />
            ) : null}
          </TabsContent>
        </Tabs>
      </PanelCard>
    </>
  )
}

/** One projection defect: what was hidden or degraded, and why (human-facing truth). */
function ProjectionRow({ issue }: { issue: GatewayProjectionIssue }) {
  const { t } = useI18n()
  if ("Down" in issue) {
    return (
      <div className="flex items-start gap-2 text-xs">
        <Badge variant="destructive">{t("gatewayProjectionIssueDown")}</Badge>
        <span className="text-muted-foreground">
          {issue.Down.service}: {issue.Down.reason}
        </span>
      </div>
    )
  }
  if ("SkippedTool" in issue) {
    return (
      <div className="flex items-start gap-2 text-xs">
        <Badge variant="outline">{t("gatewayProjectionIssueSkipped")}</Badge>
        <span className="text-muted-foreground">
          {issue.SkippedTool.tool ?? "?"} @ {issue.SkippedTool.service ?? "?"}:{" "}
          {issue.SkippedTool.reason}
        </span>
      </div>
    )
  }
  return (
    <div className="flex items-start gap-2 text-xs">
      <Badge variant="secondary">{t("gatewayProjectionIssueDegraded")}</Badge>
      <span className="text-muted-foreground">
        {issue.DegradedTool.tool} @ {issue.DegradedTool.service}
      </span>
    </div>
  )
}

/** Launch settings form: transport/host/port/path/scope, with optional port saved as the next default. */
function LaunchSettingsDialog({
  busy,
  onOpenChange,
  onStart,
  open,
  options,
  setOptions,
}: {
  busy: boolean
  onOpenChange: (open: boolean) => void
  onStart: (options: McpHubOptions, persistPort: boolean) => void
  open: boolean
  options: McpHubOptions
  setOptions: (update: (prev: McpHubOptions) => McpHubOptions) => void
}) {
  const { t } = useI18n()
  const [persistPort, setPersistPort] = useState(true)
  const isHttp = options.transport === "streamable-http"

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[min(85vh,720px)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("gatewaySettings")}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <Label>{t("gatewayTransport")}</Label>
            <Select
              value={options.transport}
              onValueChange={(value) =>
                setOptions((prev) => ({
                  ...prev,
                  transport: value as McpHubOptions["transport"],
                }))
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="streamable-http">streamable-http</SelectItem>
                <SelectItem value="stdio">stdio</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex flex-col gap-1.5">
            <Label>{t("gatewayScope")}</Label>
            <Select
              value={options.scope ?? "store"}
              onValueChange={(value) =>
                setOptions((prev) => ({ ...prev, scope: value as "store" | "agent" }))
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="store">{t("gatewayScopeStore")}</SelectItem>
                <SelectItem value="agent">{t("gatewayScopeAgent")}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {options.scope === "agent" ? (
            <div className="flex flex-col gap-1.5">
              <Label>{t("gatewayAgentId")}</Label>
              <Input
                value={options.agentId ?? ""}
                onChange={(event) =>
                  setOptions((prev) => ({ ...prev, agentId: event.target.value }))
                }
                placeholder="agent-id"
              />
            </div>
          ) : null}

          {isHttp ? (
            <>
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1.5">
                  <Label>{t("gatewayHost")}</Label>
                  <Input
                    value={options.host ?? ""}
                    onChange={(event) =>
                      setOptions((prev) => ({ ...prev, host: event.target.value }))
                    }
                    placeholder="127.0.0.1"
                  />
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label>{t("gatewayPort")}</Label>
                  <Input
                    type="number"
                    min={1}
                    max={65535}
                    value={options.port ?? ""}
                    onChange={(event) =>
                      setOptions((prev) => ({
                        ...prev,
                        port: event.target.value ? Number(event.target.value) : undefined,
                      }))
                    }
                    placeholder="1230"
                  />
                </div>
              </div>
              <div className="flex flex-col gap-1.5">
                <Label>{t("gatewayPath")}</Label>
                <Input
                  value={options.path ?? ""}
                  onChange={(event) =>
                    setOptions((prev) => ({ ...prev, path: event.target.value }))
                  }
                  placeholder="/mcp"
                />
              </div>
              <div className="flex items-center justify-between gap-2 rounded-md border border-dashed px-3 py-2">
                <Label htmlFor="gateway-persist-port" className="text-sm font-normal">
                  {t("gatewayPersistPort")}
                </Label>
                <Switch
                  id="gateway-persist-port"
                  checked={persistPort}
                  onCheckedChange={setPersistPort}
                />
              </div>
            </>
          ) : null}

          <div className="flex flex-col gap-1.5">
            <Label>{t("gatewayInstanceId")}</Label>
            <Input
              value={options.instanceId ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, instanceId: event.target.value || undefined }))
              }
              placeholder={t("gatewayInstanceIdPlaceholder")}
            />
          </div>

          <div className="flex flex-col gap-1.5">
            <Label>{t("gatewaySessionKey")}</Label>
            <Input
              value={options.sessionKey ?? ""}
              onChange={(event) =>
                setOptions((prev) => ({ ...prev, sessionKey: event.target.value || undefined }))
              }
              placeholder={t("gatewaySessionKeyPlaceholder")}
            />
          </div>

          <div className="flex flex-col gap-2">
            <Label>{t("clientsExpose")}</Label>
            <div className="grid grid-cols-2 gap-x-4 gap-y-2">
              {EXPOSE_KEYS.map((key) => (
                <div key={key} className="flex cursor-pointer items-center gap-2 text-sm">
                  <Switch
                    checked={options.expose?.[key] ?? false}
                    onCheckedChange={(checked) =>
                      setOptions((prev) => ({
                        ...prev,
                        expose: { ...prev.expose, [key]: checked },
                      }))
                    }
                  />
                  <span className="font-mono text-xs">{key}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button disabled={busy} onClick={() => onStart(options, isHttp && persistPort)}>
            {t("gatewayStart")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** Long-polling loop: waits for new records or the 25s timeout, then continues; resets and re-fetches on backend restart (seq goes backwards). */
function useGatewayRequestFlow() {
  const [records, setRecords] = useState<GatewayRequestRecord[]>([])
  useEffect(() => {
    let cancelled = false
    let after = 0
    void (async () => {
      while (!cancelled) {
        try {
          const page = await getGatewayRequests(after, 25_000)
          if (cancelled) return
          if (page.latest_seq < after) {
            after = 0
            setRecords([])
          }
          if (page.requests.length > 0) {
            after = page.latest_seq
            setRecords((prev) => [...prev, ...page.requests].slice(-200))
          } else if (!page.running) {
            // When the gateway is not running the backend returns immediately (no 25s long wait); throttle here or it spins at full speed
            await new Promise((resolve) => setTimeout(resolve, 3_000))
          }
        } catch {
          await new Promise((resolve) => setTimeout(resolve, 3_000))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])
  return records
}

const KIND_LABEL_KEYS = {
  tool: "gatewayCallKindTool",
  prompt: "gatewayCallKindPrompt",
  resource: "gatewayCallKindResource",
} as const

function FlowRow({ record }: { record: GatewayRequestRecord }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed px-3 py-2">
      <Badge variant="outline" className="shrink-0 font-mono">
        {record.caller ?? t("gatewayCallDirect")}
      </Badge>
      <Badge variant="secondary" className="shrink-0 text-[11px] uppercase">
        {t(KIND_LABEL_KEYS[record.kind])}
      </Badge>
      <WaypointsIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate font-mono text-sm">
        {record.kind === "resource" ? record.name : `${record.service_name}.${record.name}`}
      </span>
      <span className="flex shrink-0 items-center gap-2">
        <Badge variant={record.success ? "secondary" : "destructive"}>
          {record.success ? "OK" : "ERR"}
        </Badge>
        <span className="text-xs text-muted-foreground">{record.duration_ms}ms</span>
      </span>
    </div>
  )
}

/** Agent → mcpstore → MCP three-column call topology. */
function CallFlowDiagram({
  labelById,
  latest,
  status,
  wired,
}: {
  labelById: Map<string, string>
  latest: GatewayRequestRecord | null
  status: GatewayStatus | null
  wired: WiredEntry[]
}) {
  const { t } = useI18n()
  const activeCaller = latest?.caller ?? null
  const hubEndpoint =
    status?.host && status.port ? `${status.host}:${status.port}` : (status?.url ?? "—")
  const fresh = latest != null && Date.now() - latest.at_unix_ms < 4_000

  return (
    <div className="flex min-w-[720px] items-stretch gap-0">
      <div className="flex w-[148px] shrink-0 flex-col justify-center gap-2">
        {wired.length === 0 ? (
          <FlowNodeCard className="border-dashed">
            <p className="text-xs text-muted-foreground">{t("gatewayFlowEmptyAgent")}</p>
          </FlowNodeCard>
        ) : (
          wired.map((entry) => (
            <AgentFlowCard
              key={entry.client_id}
              active={activeCaller === entry.client_id}
              entry={entry}
              label={labelById.get(entry.client_id) ?? entry.client_id}
            />
          ))
        )}
      </div>

      <FlowConnector
        accent={wired.length > 0 && Boolean(status?.running)}
        from={Math.max(wired.length, 1)}
      />

      <div className="flex w-[148px] shrink-0 flex-col justify-center">
        <HubFlowCard endpoint={hubEndpoint} running={Boolean(status?.running)} />
      </div>

      <FlowConnector accent={Boolean(latest)} endDot from={1} />

      <div className="flex min-w-[180px] flex-1 flex-col justify-center">
        <ServiceFlowCard fresh={fresh} record={latest} />
      </div>
    </div>
  )
}

/** Fan-in dashed connectors: `from` left stubs → single right join. */
function FlowConnector({
  accent,
  endDot,
  from = 1,
}: {
  accent?: boolean
  endDot?: boolean
  from?: number
}) {
  const n = Math.max(from, 1)
  const vbH = 100
  const rightY = vbH / 2
  const leftYs = Array.from({ length: n }, (_, i) => ((i + 0.5) / n) * vbH)

  return (
    <div className="relative flex w-10 shrink-0 self-stretch px-1" aria-hidden>
      <svg
        className={cn("h-full w-full overflow-visible", accent ? "text-orange-400" : "text-border")}
        viewBox={`0 0 40 ${vbH}`}
        preserveAspectRatio="none"
      >
        {leftYs.map((y, i) => (
          <path
            key={i}
            d={`M 0 ${y} C 18 ${y}, 22 ${rightY}, 36 ${rightY}`}
            fill="none"
            stroke="currentColor"
            strokeDasharray="3 2.5"
            strokeWidth={1.25}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      {endDot ? (
        <span
          className={cn(
            "absolute top-1/2 right-0 size-2 -translate-y-1/2 rounded-full",
            accent ? "bg-orange-500" : "bg-border",
          )}
        />
      ) : null}
    </div>
  )
}

function FlowNodeCard({
  children,
  className,
  emphasized,
}: {
  children: ReactNode
  className?: string
  emphasized?: boolean
}) {
  return (
    <div
      className={cn(
        "relative flex flex-col gap-1.5 rounded-xl border bg-card p-3 shadow-xs",
        emphasized ? "border-foreground/80" : "border-border/70",
        className,
      )}
    >
      {children}
    </div>
  )
}

function AgentFlowCard({
  active,
  entry,
  label,
}: {
  active: boolean
  entry: WiredEntry
  label: string
}) {
  const iconUrl = clientIconUrls[entry.client_id]

  return (
    <FlowNodeCard className={cn("items-center", active && "ring-1 ring-orange-400/70")}>
      {iconUrl ? (
        <img src={iconUrl} alt="" className="size-7 object-contain" aria-hidden />
      ) : (
        <span
          aria-hidden
          className="flex size-7 items-center justify-center rounded-md bg-muted text-[10px] font-medium uppercase"
        >
          {entry.client_id.slice(0, 2)}
        </span>
      )}
      <div className="flex min-w-0 max-w-full items-center gap-1.5">
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            active ? "bg-orange-500" : "bg-emerald-500",
          )}
        />
        <span className="truncate text-sm font-semibold leading-tight">{label}</span>
      </div>
    </FlowNodeCard>
  )
}

function HubFlowCard({ endpoint, running }: { endpoint: string; running: boolean }) {
  const { t } = useI18n()
  return (
    <FlowNodeCard emphasized>
      <MarkIcon className="size-8" />
      <div className="flex min-w-0 items-center gap-1.5">
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            running ? "bg-emerald-500" : "bg-muted-foreground/40",
          )}
        />
        <strong className="truncate font-mono text-sm font-semibold">mcpstore</strong>
      </div>
      <span className="truncate font-mono text-[11px] text-muted-foreground">
        {running ? endpoint : t("gatewayStopped")}
      </span>
    </FlowNodeCard>
  )
}

function ServiceFlowCard({
  fresh,
  record,
}: {
  fresh: boolean
  record: GatewayRequestRecord | null
}) {
  const { t } = useI18n()

  if (!record) {
    return (
      <FlowNodeCard className="min-h-[88px] justify-center border-dashed">
        <p className="text-xs text-muted-foreground">{t("gatewayFlowEmptyService")}</p>
      </FlowNodeCard>
    )
  }

  return (
    <FlowNodeCard className={cn("min-h-[88px]", fresh && "ring-1 ring-orange-400/70")}>
      <div className="flex min-w-0 items-center gap-2">
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            fresh
              ? "animate-pulse bg-orange-500"
              : record.success
                ? "bg-emerald-500"
                : "bg-destructive",
          )}
        />
        <span className="flex size-7 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/50">
          <DefaultServiceIcon className="size-4" />
        </span>
        <span className="min-w-0 truncate text-sm font-semibold">{record.service_name}</span>
        <Badge variant="secondary" className="max-w-[40%] shrink-0 truncate font-mono text-[11px]">
          {record.name}
        </Badge>
      </div>
      <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
        <span className="truncate">
          {fresh
            ? t("gatewayFlowCalling")
            : record.success
              ? `${t("gatewayFlowLastCall")} · OK · ${record.duration_ms}ms`
              : `${t("gatewayFlowLastCall")} · ERR · ${record.duration_ms}ms`}
        </span>
        {record.caller ? (
          <span className="ml-auto shrink-0 truncate font-mono text-[11px]">{record.caller}</span>
        ) : (
          <span className="ml-auto shrink-0 font-mono text-[11px]">{t("gatewayCallDirect")}</span>
        )}
      </div>
    </FlowNodeCard>
  )
}
