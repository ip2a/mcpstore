import { useEffect, useRef, useState, type ReactNode } from "react"
import {
  AlertCircleIcon,
  CheckIcon,
  CircleIcon,
  SettingsIcon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"

import { clientIconUrls } from "@/assets/icons/clients"
import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { MarkIcon } from "@/components/shared/mark-icon"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import {
  WORKSPACE_LIST_INSET_CLASSNAME,
  WORKSPACE_PANEL_CLASSNAME,
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
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
  useClientsQuery,
  useGatewayStatusQuery,
  useWiredClientsQuery,
} from "@/features/clients/queries"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { useGatewayRequestFlow } from "@/features/gateway/use-gateway-request-flow"
import { useI18n } from "@/lib/i18n-context"
import {
  startMcpHub,
  stopMcpHub,
  type GatewayBoot,
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

export function GatewayView({
  onOpenScope,
}: {
  onOpenScope?: (scopeId: string) => void
} = {}) {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const clients = useClientsQuery()
  const wired = useWiredClientsQuery()
  const records = useGatewayRequestFlow()
  const meta = useSettingsMetaQuery()
  const updateSettings = useUpdateSettingsMutation()
  const [busy, setBusy] = useState<"start" | "stop" | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [launchOptions, setLaunchOptions] = useState<McpHubOptions>(DEFAULT_LAUNCH_OPTIONS)
  const boot = status.data?.boot
  const starting = status.data?.starting === true
  const failed = boot?.phase === "failed" && !starting && status.data?.running !== true
  const bootNoticePending = useRef(false)

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

  useEffect(() => {
    if (starting) {
      bootNoticePending.current = true
      return
    }
    if (!bootNoticePending.current) return
    if (status.data?.running) {
      toast.success(t("gatewayStarted"))
    } else if (failed) {
      toast.error(boot?.error || t("gatewayBootFailed"))
    }
    bootNoticePending.current = false
  }, [starting, status.data?.running, failed, boot?.error, t])

  async function handleStart(options: McpHubOptions, persistPort: boolean) {
    setBusy("start")
    try {
      await startMcpHub(options)
      bootNoticePending.current = true
      if (persistPort && options.port) {
        await updateSettings.mutateAsync({ gateway: { port: options.port } })
        await meta.refetch()
      }
      await status.refetch()
      setSettingsOpen(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(null)
    }
  }

  async function handleStop() {
    setBusy("stop")
    const toastId = toast.loading(t("gatewayStopping"))
    try {
      await stopMcpHub()
      await status.refetch()
      toast.success(t("gatewayStopped"), { id: toastId })
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error), { id: toastId })
    } finally {
      setBusy(null)
    }
  }

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("gatewayWorkspaceTitle")}</h1>}
        subtitle={t("gatewayWorkspaceDescription", { count: wiredLabels.length })}
      />

      <LaunchSettingsDialog
        busy={busy !== null || starting}
        onOpenChange={setSettingsOpen}
        onStart={handleStart}
        open={settingsOpen}
        options={launchOptions}
        setOptions={setLaunchOptions}
      />

      <PanelCard className={WORKSPACE_PANEL_CLASSNAME}>
        <div className={WORKSPACE_TABS_CLASSNAME}>
          <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
            <span className="text-sm font-medium uppercase text-foreground/60">
              {t("gatewayRequestsTitle")}
            </span>
            <div className="ml-auto flex min-w-0 shrink-0 flex-wrap items-center justify-end gap-2">
              {status.data?.running ? (
                <Badge variant="default">{t("homeRunning")}</Badge>
              ) : starting ? (
                <Badge variant="secondary">{t("gatewayStarting")}</Badge>
              ) : (
                <Badge variant="destructive">
                  {failed ? t("gatewayBootFailed") : t("gatewayStopped")}
                </Badge>
              )}
              <Button
                disabled={busy !== null || starting}
                size="sm"
                variant="outline"
                onClick={() => setSettingsOpen(true)}
              >
                <SettingsIcon data-icon="inline-start" />
                {t("gatewaySettings")}
              </Button>
              {status.data?.running || starting ? (
                <Button disabled={busy !== null} size="sm" variant="outline" onClick={handleStop}>
                  {busy === "stop" ? <Spinner data-icon="inline-start" /> : null}
                  {busy === "stop" ? t("gatewayStopping") : t("gatewayStop")}
                </Button>
              ) : (
                <Button
                  disabled={busy !== null}
                  size="sm"
                  onClick={() => handleStart(launchOptions, false)}
                >
                  {busy === "start" ? <Spinner data-icon="inline-start" /> : null}
                  {busy === "start" ? t("gatewayStarting") : t("gatewayStart")}
                </Button>
              )}
            </div>
          </div>

          {starting || failed ? <GatewayBootProgress boot={boot} failed={failed} /> : null}

          <div
            className={cn(
              WORKSPACE_LIST_INSET_CLASSNAME,
              "flex min-h-0 flex-1 flex-col gap-3 overflow-hidden",
            )}
          >
            <CallFlowDiagram
              labelById={labelById}
              onOpenScope={onOpenScope}
              recent={[...records].slice(-5).reverse()}
              status={status.data ?? null}
              wired={wired.data ?? []}
            />

            <div className="shrink-0 border-t border-border/50" />

            <div className="flex shrink-0 flex-wrap items-center justify-between gap-4 py-2">
              <p className="min-w-0 flex-1 text-sm text-muted-foreground">
                {t("gatewayFlowOverviewBlurb")}
              </p>
              <div className="flex shrink-0 items-end gap-6 sm:gap-8">
                {(
                  [
                    {
                      value: records.length,
                      label: t("gatewayFlowStatRequests"),
                    },
                    {
                      value: new Set(records.map((r) => r.service_name)).size,
                      label: t("gatewayFlowStatServices"),
                    },
                    {
                      value: records.filter((r) => r.success).length,
                      label: t("gatewayFlowStatSuccess"),
                    },
                  ] as const
                ).map((stat) => (
                  <div key={stat.label} className="flex flex-col items-center gap-0.5">
                    <span className="text-2xl font-semibold tabular-nums leading-none">
                      {stat.value}
                    </span>
                    <span className="text-xs text-muted-foreground">{stat.label}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="shrink-0 border-t border-border/50" />

            <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden py-1">
              <h3 className="shrink-0 text-sm font-semibold">{t("gatewayFlowLatestTitle")}</h3>
              <div className="min-h-0 flex-1 overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
                <LatestCallNarrative />
              </div>
            </div>
          </div>
        </div>
      </PanelCard>
    </>
  )
}

const BOOT_STEP_LABEL_KEYS = [
  "gatewayBootSpawn",
  "gatewayBootMounts",
  "gatewayBootReady",
] as const
const BOOT_STEP_INDEX = {
  spawning: 0,
  mounting: 1,
  binding: 2,
  ready: 2,
  failed: 0,
} as const

function GatewayBootProgress({ boot, failed }: { boot?: GatewayBoot | null; failed: boolean }) {
  const { t } = useI18n()
  if (failed) {
    return (
      <Alert
        variant="destructive"
        className="shrink-0 rounded-none border-x-0 border-t-0 px-4 py-2"
      >
        <AlertCircleIcon />
        <AlertTitle>{t("gatewayBootFailed")}</AlertTitle>
        <AlertDescription className="line-clamp-2">
          {boot?.error || t("gatewayBootFailedDescription")}
        </AlertDescription>
      </Alert>
    )
  }
  if (!boot) {
    return (
      <div
        role="status"
        aria-live="polite"
        className="flex shrink-0 items-center gap-2 border-b bg-muted/20 px-4 py-2 text-xs"
      >
        <Spinner />
        {t("gatewayStarting")}
      </div>
    )
  }
  const currentStep = BOOT_STEP_INDEX[boot.phase]
  return (
    <div
      role="status"
      aria-live="polite"
      className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b bg-muted/20 px-4 py-2 text-xs"
    >
      {BOOT_STEP_LABEL_KEYS.map((labelKey, index) => (
        <span
          key={labelKey}
          className={cn(
            "flex items-center gap-2",
            index === currentStep
              ? "text-foreground"
              : index < currentStep
                ? "text-muted-foreground"
                : "text-muted-foreground/50",
          )}
        >
          {index < currentStep ? (
            <CheckIcon className="size-3.5" />
          ) : index === currentStep ? (
            <Spinner />
          ) : (
            <CircleIcon className="size-3.5" />
          )}
          <span>{t(labelKey)}</span>
        </span>
      ))}
      {boot.message ? (
        <span className="max-w-48 truncate font-mono text-muted-foreground" title={boot.message}>
          {boot.message}
        </span>
      ) : null}
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

          {/*
            Management-tool expose toggles used to live here. That capability is NOT
            part of the gateway: it belongs to a future standalone "mcpstore management
            MCP" (Settings → Management MCP). Do not re-attach expose flags to hub start.
          */}
        </div>
        <DialogFooter>
          <Button disabled={busy} onClick={() => onStart(options, isHttp && persistPort)}>
            {busy ? <Spinner data-icon="inline-start" /> : null}
            {busy ? t("gatewayStarting") : t("gatewayStart")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** Narrative narrative markers for the latest-call story. */
type NarrativeKind = "muted" | "emphasis" | "ok" | "error" | "detail"

/** Temporary mock — replace with real latest-call narrative later. */
const MOCK_LATEST_NARRATIVE: { kind: NarrativeKind; text: string }[] = [
  { kind: "muted", text: "Claude Code call 工具：demo1_add" },
  { kind: "emphasis", text: "mcpstore 路由给服务 demo1 的工具 demo1__add" },
  { kind: "muted", text: "没有映射，直接透传" },
  { kind: "ok", text: "上游返回成功 · 24ms" },
  { kind: "detail", text: "原话：demo1.demo1__add ok" },
]

/** Temporary mock timing segments — Narrative latency breakdown. */
const MOCK_TIMING_SEGMENTS: { key: string; label: string; detail: string; ms: number; color: string }[] =
  [
    {
      key: "hub",
      label: "mcpstore",
      detail: "mcpstore 内耗时",
      ms: 2,
      color: "bg-sky-500",
    },
    {
      key: "net",
      label: "网络",
      detail: "发送请求",
      ms: 8,
      color: "bg-amber-700",
    },
    {
      key: "svc",
      label: "服务",
      detail: "demo1（首字节）",
      ms: 12,
      color: "bg-zinc-700",
    },
    {
      key: "read",
      label: "读取回复",
      detail: "读取回复",
      ms: 2,
      color: "bg-zinc-400",
    },
  ]

function NarrativeMarker({ kind }: { kind: NarrativeKind }) {
  if (kind === "error") {
    return <XIcon className="mt-0.5 size-3.5 shrink-0 text-destructive" aria-hidden />
  }
  if (kind === "detail") {
    return (
      <span
        aria-hidden
        className="mt-1 size-2.5 shrink-0 rounded-full border border-muted-foreground/70"
      />
    )
  }
  return (
    <span
      aria-hidden
      className={cn(
        "mt-1 size-2.5 shrink-0 rounded-full",
        kind === "muted" && "bg-muted-foreground/50",
        kind === "emphasis" && "bg-foreground",
        kind === "ok" && "bg-emerald-500",
      )}
    />
  )
}

function TimingBreakdown() {
  const { t } = useI18n()
  const total = MOCK_TIMING_SEGMENTS.reduce((sum, seg) => sum + seg.ms, 0) || 1

  return (
    <li className="flex items-start gap-2.5">
      <NarrativeMarker kind="detail" />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <span className="text-sm font-medium">{t("gatewayFlowTimingTitle")}</span>
        <div className="flex h-2 w-full overflow-hidden rounded-sm bg-muted">
          {MOCK_TIMING_SEGMENTS.map((seg) => (
            <div
              key={seg.key}
              className={cn("h-full", seg.color)}
              style={{ width: `${(seg.ms / total) * 100}%` }}
              title={`${seg.detail} ${seg.ms}ms`}
            />
          ))}
        </div>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-xs text-muted-foreground">
          {MOCK_TIMING_SEGMENTS.map((seg) => (
            <span key={seg.key} className="inline-flex items-center gap-1.5">
              <span className={cn("size-2.5 shrink-0 rounded-sm", seg.color)} aria-hidden />
              {seg.detail} {t("gatewayFlowTimingMs", { ms: seg.ms })}
            </span>
          ))}
        </div>
      </div>
    </li>
  )
}

function LatestCallNarrative() {
  const stepsBeforeRaw = MOCK_LATEST_NARRATIVE.slice(0, -1)
  const rawStep = MOCK_LATEST_NARRATIVE[MOCK_LATEST_NARRATIVE.length - 1]!

  return (
    <ul className="flex flex-col gap-2.5 text-sm">
      {stepsBeforeRaw.map((step, index) => (
        <li key={index} className="flex items-start gap-2.5">
          <NarrativeMarker kind={step.kind} />
          <span
            className={cn(
              "min-w-0 leading-snug",
              step.kind === "detail" && "font-mono text-xs text-muted-foreground",
              step.kind === "error" && "text-destructive",
            )}
          >
            {step.text}
          </span>
        </li>
      ))}
      <TimingBreakdown />
      <li className="flex items-start gap-2.5">
        <NarrativeMarker kind={rawStep.kind} />
        <span className="min-w-0 font-mono text-xs leading-snug text-muted-foreground">
          {rawStep.text}
        </span>
      </li>
    </ul>
  )
}

function scopeIdFromWired(entry: WiredEntry): string {
  return entry.scope.type === "store" ? "store" : entry.scope.agent_id
}

/** Agent → mcpstore → MCP three-column call topology. */
function CallFlowDiagram({
  labelById,
  onOpenScope,
  recent,
  status,
  wired,
}: {
  labelById: Map<string, string>
  onOpenScope?: (scopeId: string) => void
  recent: GatewayRequestRecord[]
  status: GatewayStatus | null
  wired: WiredEntry[]
}) {
  const { t } = useI18n()
  const latest = recent[0] ?? null
  const activeCaller = latest?.caller ?? null
  const activeWired = activeCaller
    ? wired.find((entry) => entry.client_id === activeCaller)
    : undefined
  const hubUrl = activeWired?.url ?? status?.url ?? "—"
  const fresh = latest != null && Date.now() - latest.at_unix_ms < 4_000

  return (
    <div className="flex w-full min-w-0 items-stretch gap-0">
      <div className="flex min-w-0 flex-[0.65] basis-0 flex-col justify-center gap-2">
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
              onOpen={onOpenScope ? () => onOpenScope(scopeIdFromWired(entry)) : undefined}
            />
          ))
        )}
      </div>

      <FlowConnector
        accent={wired.length > 0 && Boolean(status?.running)}
        from={Math.max(wired.length, 1)}
      />

      <div className="flex min-w-0 flex-[0.9] basis-0 flex-col justify-center">
        <HubFlowCard endpoint={hubUrl} running={Boolean(status?.running)} />
      </div>

      <FlowConnector accent={Boolean(latest)} endDot from={1} />

      <div className="flex min-w-0 flex-[1.3] basis-0 flex-col justify-center">
        <ServiceFlowCard fresh={fresh} recent={recent} />
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
  onClick,
}: {
  children: ReactNode
  className?: string
  emphasized?: boolean
  onClick?: () => void
}) {
  if (onClick) {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "relative flex w-full min-w-0 flex-col gap-1.5 rounded-xl border bg-card p-3 text-left shadow-xs outline-none",
          "focus-visible:ring-[3px] focus-visible:ring-ring/50",
          emphasized ? "border-foreground/80" : "border-border/70",
          className,
        )}
      >
        {children}
      </button>
    )
  }

  return (
    <div
      className={cn(
        "relative flex w-full min-w-0 flex-col gap-1.5 rounded-xl border bg-card p-3 shadow-xs",
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
  onOpen,
}: {
  active: boolean
  entry: WiredEntry
  label: string
  onOpen?: () => void
}) {
  const iconUrl = clientIconUrls[entry.client_id]

  return (
    <FlowNodeCard
      className={cn(
        "items-center",
        active && "ring-1 ring-orange-400/70",
        onOpen && "cursor-pointer transition-colors hover:bg-muted/40",
      )}
      onClick={onOpen}
    >
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
    <FlowNodeCard emphasized className="items-center text-center">
      <MarkIcon className="size-8" />
      <div className="flex min-w-0 max-w-full items-center justify-center gap-1.5">
        <span
          className={cn(
            "size-1.5 shrink-0 rounded-full",
            running ? "bg-emerald-500" : "bg-muted-foreground/40",
          )}
        />
        <strong className="truncate font-mono text-sm font-semibold">mcpstore</strong>
      </div>
      <span
        className="max-w-full truncate font-mono text-[11px] text-muted-foreground"
        title={running ? endpoint : undefined}
      >
        {running ? endpoint : t("gatewayStopped")}
      </span>
    </FlowNodeCard>
  )
}

function ServiceFlowCard({
  fresh,
  recent,
}: {
  fresh: boolean
  recent: GatewayRequestRecord[]
}) {
  const { t } = useI18n()
  const latest = recent[0] ?? null

  if (!latest) {
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
              : latest.success
                ? "bg-emerald-500"
                : "bg-destructive",
          )}
        />
        <span className="flex size-7 shrink-0 items-center justify-center text-muted-foreground">
          <DefaultServiceIcon className="size-7" />
        </span>
        <span className="min-w-0 truncate text-sm font-semibold">{latest.service_name}</span>
        <Badge variant="secondary" className="max-w-[40%] shrink-0 truncate font-mono text-[11px]">
          {latest.name}
        </Badge>
      </div>
      <div className="border-t border-border/50" />
      <div className="flex min-w-0 flex-col gap-1">
        {recent.map((record, index) => (
          <div
            key={record.seq}
            className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground"
          >
            <span className="min-w-0 truncate font-mono text-[11px]">{record.name}</span>
            <span className="shrink-0">
              {index === 0 && fresh
                ? t("gatewayFlowCalling")
                : `${record.success ? "OK" : "ERR"} · ${record.duration_ms}ms`}
            </span>
            <span className="ml-auto shrink-0 truncate font-mono text-[11px]">
              {record.caller ?? t("gatewayCallDirect")}
            </span>
          </div>
        ))}
      </div>
    </FlowNodeCard>
  )
}
