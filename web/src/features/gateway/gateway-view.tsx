import { useEffect, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CableIcon,
  PencilIcon,
  RefreshCwIcon,
  SettingsIcon,
  WaypointsIcon,
} from "lucide-react"
import { toast } from "sonner"

import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
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
import { EXPOSE_KEYS } from "@/features/clients/clients-view"
import {
  useClientsQuery,
  useGatewayStatusQuery,
  useWireClientMutation,
  useWiredClientsQuery,
} from "@/features/clients/queries"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import {
  getFrameSessions,
  getFrameHistory,
  getGatewayRequests,
  getRequestHistory,
  startMcpHub,
  stopMcpHub,
  type ExposeConfig,
  type FrameRecord,
  type FrameSessionSummary,
  type GatewayRequestRecord,
  type McpHubOptions,
  type RequestRecord,
  type ScopeRef,
  type WiredEntry,
} from "@/lib/api"

const DEFAULT_LAUNCH_OPTIONS: McpHubOptions = {
  transport: "streamable-http",
  scope: "store",
  host: "127.0.0.1",
  path: "/mcp",
}

export function GatewayView() {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const clients = useClientsQuery()
  const wired = useWiredClientsQuery()
  const records = useGatewayRequestFlow()
  const meta = useSettingsMetaQuery()
  const updateSettings = useUpdateSettingsMutation()
  const [busy, setBusy] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [launchOptions, setLaunchOptions] = useState<McpHubOptions>(DEFAULT_LAUNCH_OPTIONS)

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

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      <LaunchSettingsDialog
        busy={busy}
        onOpenChange={setSettingsOpen}
        onStart={handleStart}
        open={settingsOpen}
        options={launchOptions}
        setOptions={setLaunchOptions}
      />

      <PanelCard variant="plain" className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b pb-3">
          <div className="flex items-center gap-2">
            <strong className="font-mono text-sm font-medium">{t("gateway")}</strong>
            {status.data?.running ? (
              <Badge variant="default">{t("homeRunning")}</Badge>
            ) : (
              <Badge variant="destructive">{t("gatewayStopped")}</Badge>
            )}
          </div>
          <div className="flex gap-2">
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
        <div className="flex flex-col gap-1 font-mono text-xs text-muted-foreground">
          <span className="truncate">
            {t("gatewayUrl")}: {status.data?.url ?? "—"}
          </span>
          <span className="truncate">
            {t("clientsWired")}: {wiredLabels.length > 0 ? wiredLabels.join(", ") : "—"}
          </span>
        </div>
      </PanelCard>

      <WiredClientsCard labelById={labelById} wired={wired.data ?? []} />

      <PanelCard variant="plain" className="gap-2">
        <div className="border-b pb-2 text-sm font-medium">{t("gatewayRequestsTitle")}</div>
        {records.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("gatewayRequestsEmpty")}</p>
        ) : (
          [...records].reverse().map((record) => <FlowRow key={record.seq} record={record} />)
        )}
      </PanelCard>

      <RequestHistoryCard />

      <FrameStreamCard />
    </ScrollPane>
  )
}

/** 启动参数表单：transport/host/port/path/scope，可选把 port 存为下次默认值。 */
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
                <label key={key} className="flex cursor-pointer items-center gap-2 text-sm">
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
                </label>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button
            disabled={busy}
            onClick={() => onStart(options, isHttp && persistPort)}
          >
            {t("gatewayStart")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** MCP 消息流：网关与对外连接的 JSON-RPC 帧会话（~/.mcpstore/telemetry.db frames 表）。 */
function FrameStreamCard() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [active, setActive] = useState<string | null>(null)
  const [openFrame, setOpenFrame] = useState<number | null>(null)
  const sessions = useQuery({
    queryKey: queryKeys.gatewayFrameSessions,
    queryFn: () => getFrameSessions({ hours: 24 }),
  })
  const timeline = useQuery({
    queryKey: queryKeys.gatewayFrameHistory(active ?? ""),
    enabled: active !== null,
    queryFn: () => getFrameHistory(active as string),
  })

  const groups: { face: FrameSessionSummary["face"]; key: string }[] = [
    { face: "gateway", key: "gatewayFramesFaceGateway" },
    { face: "upstream", key: "gatewayFramesFaceUpstream" },
  ]
  const all = sessions.data ?? []

  function refresh() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayFrameSessions })
    if (active) {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.gatewayFrameHistory(active),
      })
    }
  }

  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="flex items-center justify-between border-b pb-2">
        <div className="text-sm font-medium">{t("gatewayFramesTitle")}</div>
        <Button
          disabled={sessions.isFetching}
          size="sm"
          variant="ghost"
          onClick={refresh}
        >
          <RefreshCwIcon className="size-3.5" />
        </Button>
      </div>
      {all.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayFramesEmpty")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {groups.map(({ face, key }) => {
            const items = all.filter((session) => session.face === face)
            if (items.length === 0) return null
            return (
              <div key={face} className="flex flex-col gap-1">
                <div className="text-xs font-medium text-muted-foreground">
                  {t(key)}
                </div>
                {items.map((session) => (
                  <SessionRow
                    active={session.session_id === active}
                    key={session.session_id}
                    onSelect={() => {
                      setActive(session.session_id)
                      setOpenFrame(null)
                    }}
                    session={session}
                  />
                ))}
              </div>
            )
          })}
          {active && timeline.data ? (
            <div className="flex flex-col gap-1 border-t pt-2">
              {timeline.data.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  {t("gatewayFramesNoFrames")}
                </p>
              ) : (
                timeline.data.map((frame) => (
                  <FrameRow
                    frame={frame}
                    key={frame.id}
                    open={openFrame === frame.id}
                    onToggle={() =>
                      setOpenFrame((prev) => (prev === frame.id ? null : frame.id))
                    }
                  />
                ))
              )}
            </div>
          ) : null}
        </div>
      )}
    </PanelCard>
  )
}

function SessionRow({
  session,
  active,
  onSelect,
}: {
  session: FrameSessionSummary
  active: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      className={`flex w-full flex-wrap items-center gap-2 rounded-md border px-3 py-2 text-left ${
        active ? "border-solid bg-muted/50" : "border-dashed"
      }`}
      onClick={onSelect}
    >
      <Badge variant="outline" className="shrink-0 font-mono">
        {session.peer ?? session.caller ?? session.session_id.slice(0, 12)}
      </Badge>
      <span className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
        {session.instance_id ?? session.caller ?? "—"}
      </span>
      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
        {new Date(session.last_ts_unix_ms).toLocaleTimeString()}
      </span>
      <Badge variant="secondary" className="shrink-0">
        {session.frame_count}
      </Badge>
    </button>
  )
}

function FrameRow({
  frame,
  open,
  onToggle,
}: {
  frame: FrameRecord
  open: boolean
  onToggle: () => void
}) {
  const { t } = useI18n()
  const Arrow = frame.direction === "up" ? ArrowUpIcon : ArrowDownIcon
  return (
    <div className="rounded-md border border-dashed">
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2 text-left"
        onClick={onToggle}
      >
        <Arrow
          className={`size-3.5 shrink-0 ${
            frame.direction === "up" ? "text-blue-500" : "text-emerald-500"
          }`}
        />
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {new Date(frame.ts_unix_ms).toLocaleTimeString()}
        </span>
        <span className="min-w-0 flex-1 truncate font-mono text-sm">
          {frame.method ?? `#${frame.jsonrpc_id ?? "—"}`}
        </span>
        <Badge
          variant={
            frame.kind === "error"
              ? "destructive"
              : frame.kind === "notification"
                ? "outline"
                : "secondary"
          }
          className="shrink-0"
        >
          {frame.kind}
        </Badge>
        <span className="shrink-0 text-xs text-muted-foreground">
          {frame.size_bytes}B
        </span>
      </button>
      {open ? (
        <div className="border-t px-3 py-2 text-xs">
          <PayloadBlock
            payload={frame.payload}
            title={`${frame.face} / ${frame.kind}`}
            truncated={frame.truncated}
          />
        </div>
      ) : null}
    </div>
  )
}

/** 长轮询循环：等新记录或 25s 超时后继续；后端重启（seq 回退）时清空重追。 */
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
            // 网关未运行时后端立即返回（不走 25s 长等），必须节流否则全速空转
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

function FlowRow({ record }: { record: GatewayRequestRecord }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed px-3 py-2">
      <Badge variant="outline" className="shrink-0 font-mono">
        {record.caller ?? t("gatewayCallDirect")}
      </Badge>
      <WaypointsIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate font-mono text-sm">
        {record.service_name}.{record.tool}
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

/** 已接入客户端：每个挂载点的作用域 + 暴露配置（/gateway/wired 返回生效值）。 */
function WiredClientsCard({
  labelById,
  wired,
}: {
  labelById: Map<string, string>
  wired: WiredEntry[]
}) {
  const { t } = useI18n()
  const [editing, setEditing] = useState<WiredEntry | null>(null)
  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="border-b pb-2 text-sm font-medium">{t("gatewayWiredTitle")}</div>
      {wired.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayWiredEmpty")}</p>
      ) : (
        wired.map((entry) => (
          <WiredClientRow
            key={entry.client_id}
            entry={entry}
            labelById={labelById}
            onEdit={() => setEditing(entry)}
          />
        ))
      )}
      <WireEditDialog
        entry={editing}
        labelById={labelById}
        onOpenChange={(open) => !open && setEditing(null)}
      />
    </PanelCard>
  )
}

function WiredClientRow({
  entry,
  labelById,
  onEdit,
}: {
  entry: WiredEntry
  labelById: Map<string, string>
  onEdit: () => void
}) {
  const { t } = useI18n()
  const exposeOn = Object.entries(entry.expose)
    .filter(([, on]) => on)
    .map(([key]) => key)
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed px-3 py-2">
      <CableIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 truncate text-sm font-medium">
        {labelById.get(entry.client_id) ?? entry.client_id}
      </span>
      <Badge variant="secondary" className="shrink-0 font-mono text-xs">
        {entry.scope.type === "agent" ? `Agent:${entry.scope.agent_id}` : t("gatewayScopeStore")}
      </Badge>
      {exposeOn.length > 0 ? (
        <span className="min-w-0 truncate font-mono text-xs text-muted-foreground">
          {exposeOn.join(", ")}
        </span>
      ) : (
        <span className="min-w-0 truncate text-xs text-muted-foreground">
          {t("gatewayExposeNone")}
        </span>
      )}
      <span className="min-w-0 truncate font-mono text-xs text-muted-foreground">
        {entry.url}
      </span>
      <Button
        className="ml-auto shrink-0"
        size="icon-sm"
        variant="ghost"
        onClick={onEdit}
        aria-label={t("gatewayEditWire")}
      >
        <PencilIcon className="size-3.5" />
      </Button>
    </div>
  )
}

/** 编辑已接入客户端：重新调用 /gateway/wire（幂等）覆盖 scope + 8 个暴露开关。 */
function WireEditDialog({
  entry,
  labelById,
  onOpenChange,
}: {
  entry: WiredEntry | null
  labelById: Map<string, string>
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const wire = useWireClientMutation()
  const [scope, setScope] = useState<ScopeRef>({ type: "store" })
  const [expose, setExpose] = useState<ExposeConfig | null>(null)

  useEffect(() => {
    if (entry) {
      setScope(entry.scope)
      setExpose(entry.expose)
    }
  }, [entry])

  if (!entry || !expose) {
    return <Dialog open={false} onOpenChange={onOpenChange} />
  }

  async function handleSave() {
    try {
      await wire.mutateAsync({
        client: entry!.client_id,
        options: {
          scope: scope.type,
          agentId: scope.type === "agent" ? scope.agent_id : undefined,
          expose: expose!,
        },
      })
      toast.success(t("clientsWireOk", { client: labelById.get(entry!.client_id) ?? entry!.client_id }))
      onOpenChange(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }

  return (
    <Dialog open={Boolean(entry)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {t("gatewayEditWireTitle", {
              client: labelById.get(entry.client_id) ?? entry.client_id,
            })}
          </DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <Label>{t("gatewayScope")}</Label>
            <Select
              value={scope.type}
              onValueChange={(value) =>
                setScope(
                  value === "agent"
                    ? { type: "agent", agent_id: scope.type === "agent" ? scope.agent_id : entry.client_id }
                    : { type: "store" },
                )
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
          {scope.type === "agent" ? (
            <div className="flex flex-col gap-1.5">
              <Label>{t("gatewayAgentId")}</Label>
              <Input
                value={scope.agent_id}
                onChange={(event) => setScope({ type: "agent", agent_id: event.target.value })}
                placeholder="agent-id"
              />
            </div>
          ) : null}
          <div className="flex flex-col gap-2">
            <Label>{t("clientsExpose")}</Label>
            <div className="grid grid-cols-2 gap-x-4 gap-y-2">
              {EXPOSE_KEYS.map((key) => (
                <label key={key} className="flex cursor-pointer items-center gap-2 text-sm">
                  <Switch
                    checked={expose[key]}
                    onCheckedChange={(checked) =>
                      setExpose((prev) => (prev ? { ...prev, [key]: checked } : prev))
                    }
                  />
                  <span className="font-mono text-xs">{key}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button disabled={wire.isPending} onClick={handleSave}>
            {t("save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** 历史请求：daemon 遥测持久层最近 50 条（工具调用 + 管理操作），点击行展开完整参数/返回。 */
function RequestHistoryCard() {
  const { t } = useI18n()
  const history = useQuery({
    queryKey: queryKeys.gatewayHistory,
    queryFn: () => getRequestHistory({ limit: 50 }),
    refetchInterval: 15_000,
  })
  const records = history.data?.requests ?? []
  return (
    <PanelCard variant="plain" className="gap-2">
      <div className="border-b pb-2 text-sm font-medium">{t("gatewayHistoryTitle")}</div>
      {records.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("gatewayHistoryEmpty")}</p>
      ) : (
        <div className="flex flex-col gap-1">
          {records.map((record) => (
            <HistoryRow key={record.id} record={record} />
          ))}
        </div>
      )}
    </PanelCard>
  )
}

function HistoryRow({ record }: { record: RequestRecord }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  return (
    <div className="rounded-md border border-dashed">
      <button
        type="button"
        className="flex w-full flex-wrap items-center gap-2 px-3 py-2 text-left"
        onClick={() => setOpen((prev) => !prev)}
      >
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {new Date(record.started_at_unix_ms).toLocaleTimeString()}
        </span>
        <Badge variant="outline" className="shrink-0 font-mono">
          {record.caller ?? t("gatewayCallDirect")}
        </Badge>
        <span className="min-w-0 flex-1 truncate font-mono text-sm">
          {record.kind === "tool"
            ? `${record.service_name ?? "—"}.${record.tool ?? "—"}`
            : record.tool ?? "—"}
        </span>
        <Badge variant="secondary" className="shrink-0">
          {record.kind === "tool" ? t("gatewayKindTool") : t("gatewayKindManagement")}
        </Badge>
        <Badge variant={record.success ? "secondary" : "destructive"} className="shrink-0">
          {record.success ? "OK" : "ERR"}
        </Badge>
        <span className="shrink-0 text-xs text-muted-foreground">{record.duration_ms}ms</span>
      </button>
      {open ? (
        <div className="flex flex-col gap-2 border-t px-3 py-2 text-xs">
          {record.error_summary ? (
            <div>
              <span className="font-medium text-destructive">
                {record.error_code ?? "ERR"}
              </span>
              <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2">
                {record.error_summary}
              </pre>
            </div>
          ) : null}
          <PayloadBlock
            title={t("gatewayDetailArgs")}
            payload={record.args_json}
            truncated={record.args_truncated}
          />
          <PayloadBlock
            title={t("gatewayDetailResult")}
            payload={record.result_json}
            truncated={record.result_truncated}
          />
        </div>
      ) : null}
    </div>
  )
}

function PayloadBlock({
  title,
  payload,
  truncated,
}: {
  title: string
  payload: string | null
  truncated: boolean
}) {
  const { t } = useI18n()
  if (!payload) return null
  let pretty = payload
  try {
    pretty = JSON.stringify(JSON.parse(payload), null, 2)
  } catch {
    // 非 JSON（截断残留）原样展示
  }
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="font-medium">{title}</span>
        {truncated ? (
          <Badge variant="outline" className="text-[10px]">
            {t("gatewayTruncated")}
          </Badge>
        ) : null}
      </div>
      <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2">
        {pretty}
      </pre>
    </div>
  )
}
