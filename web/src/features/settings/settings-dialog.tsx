import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ActivityIcon, AlertCircleIcon, CheckIcon, FileTextIcon, InfoIcon, MonitorIcon, NetworkIcon, PencilIcon, PlusIcon, RefreshCwIcon, SaveIcon, ServerIcon, SettingsIcon, SlidersHorizontalIcon, Trash2Icon, type LucideIcon } from "lucide-react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { WorkspaceIdentity } from "@/components/shared/workspace-identity"
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldContent, FieldDescription, FieldGroup, FieldTitle } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { payloadFromDraft, sections, settingsDraft, type ConnectionDraft, type SectionId, type SettingsDraft } from "@/features/settings/model"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { type UiLanguage } from "@/lib/api"
import { getApiBase, resolveActiveConnectionUrl, setApiBase, setConnections } from "@/lib/api/backend"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"
import { queryKeys } from "@/lib/query-keys"

const sectionIcons: Record<SectionId, LucideIcon> = {
  overview: ServerIcon,
  general: SlidersHorizontalIcon,
  connection: NetworkIcon,
  diagnostics: ActivityIcon,
  config: FileTextIcon,
  about: InfoIcon,
}

const compactInputGroupClass = "w-20 shrink-0 !w-20"

export function SettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { setLanguageOverride, t } = useI18n()
  const queryClient = useQueryClient()
  const [section, setSection] = useState<SectionId>("general")
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const metaQuery = useSettingsMetaQuery(open)
  const settingsMutation = useUpdateSettingsMutation()
  const meta = metaQuery.data
  const loading = metaQuery.isFetching && !draft
  const error = metaQuery.error instanceof Error ? metaQuery.error.message : ""
  const saving = settingsMutation.isPending

  const configFile = meta?.config_file
  const configContent = useMemo(() => configFile?.content || "", [configFile?.content])

  useEffect(() => {
    if (open && meta) setDraft(settingsDraft(meta.settings))
  }, [meta, open])

  function patchDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => (current ? { ...current, ...patch } : current))
  }

  function patchDiagnostics(patch: Partial<SettingsDraft["diagnostics"]>) {
    setDraft((current) => (current ? { ...current, diagnostics: { ...current.diagnostics, ...patch } } : current))
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!draft) return
    const activeUrl = resolveActiveConnectionUrl(draft.connections, draft.activeConnectionId)
    const apiBaseChanged = activeUrl.trim() !== getApiBase()
    try {
      await settingsMutation.mutateAsync(payloadFromDraft(draft))
      setLanguageOverride(draft.language)
      setConnections(draft.connections)
      if (apiBaseChanged) {
        setApiBase(activeUrl)
        toast.success(t("coreBackendApplied"))
        window.location.reload()
        return
      }
      toast.success(t("saved"))
      await queryClient.invalidateQueries({ queryKey: queryKeys.meta })
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("saveFailed"))
    }
  }

  function handleOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      setSection("general")
    }
    onOpenChange(nextOpen)
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex h-[min(78vh,640px)] !w-[94vw] !max-w-[94vw] flex-col gap-0 p-0 sm:!w-[72vw] sm:!max-w-[72vw]">
        <DialogHeader className="border-b px-4 py-4 sm:px-5">
          <DialogTitle className="flex items-center gap-2">
            <SettingsIcon className="size-4" />
            {t("settings")}
          </DialogTitle>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-[144px_minmax(0,1fr)]">
          <nav className="flex flex-col gap-1 border-r p-3" aria-label={t("settingsNav")}>
            {sections.map((item) => {
              const Icon = sectionIcons[item.id]
              return (
                <Button
                  key={item.id}
                  type="button"
                  variant={section === item.id ? "secondary" : "ghost"}
                  className="justify-start"
                  onClick={() => setSection(item.id)}
                >
                  <Icon data-icon="inline-start" />
                  {t(item.labelKey)}
                </Button>
              )
            })}
          </nav>

          <ScrollArea className="min-h-0">
            <div className="p-4 sm:p-5">
              {loading ? <SettingsLoading label={t("loadingSettings")} /> : null}
              {error ? <SettingsError message={error} onRetry={() => void metaQuery.refetch()} /> : null}

              {!loading && !error && draft ? (
                <DialogForm id="settings-form" onSubmit={onSubmit}>
                  {section === "overview" ? <OverviewSection draft={draft} /> : null}

                  {section === "connection" ? (
                    <ConnectionSection draft={draft} patchDraft={patchDraft} />
                  ) : null}

                  {section === "general" ? (
                    <section className="flex flex-col gap-5">
                      <SectionHead title={t("general")} />
                      <FieldGroup>
                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("language")}</FieldTitle>
                            <FieldDescription>{t("chooseLanguage")}</FieldDescription>
                          </FieldContent>
                          <Select value={draft.language} onValueChange={(value) => patchDraft({ language: value as UiLanguage })}>
                            <SelectTrigger className="w-44">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectGroup>
                                <SelectItem value="auto">{t("auto")}</SelectItem>
                                <SelectItem value="zh">{t("chinese")}</SelectItem>
                                <SelectItem value="en">{t("english")}</SelectItem>
                              </SelectGroup>
                            </SelectContent>
                          </Select>
                        </Field>
                      </FieldGroup>

                      <SectionHead title={t("coreBackend")} />
                      <BackendFields draft={draft} patchDraft={patchDraft} />
                    </section>
                  ) : null}

                  {section === "config" ? (
                    <section className="flex flex-col gap-4">
                      <SectionHead title={t("configFile")} description={t("configReadonlyDescription")} />
                      <WorkspaceIdentity
                        workspace={configFile?.path}
                        fallbackTitle={t("configFileMissing")}
                        label="Config File"
                        className="rounded-md border p-3"
                      />
                      <InputGroup>
                        <InputGroupTextarea className="min-h-80 font-mono text-xs" readOnly value={configContent} />
                      </InputGroup>
                    </section>
                  ) : null}

                  {section === "diagnostics" ? (
                    <section className="flex flex-col gap-5">
                      <SectionHead title={t("diagnostics")} description={t("diagnosticsDescription")} />
                      <FieldGroup>
                        <ToggleField title={t("diagnosticsEnabled")} description={t("diagnosticsEnabledDescription")} checked={draft.diagnostics.enabled} onCheckedChange={(enabled) => patchDiagnostics({ enabled })} />
                        <ToggleField title={t("runtimeLogEnabled")} description={t("runtimeLogEnabledDescription")} checked={draft.diagnostics.runtime_enabled} disabled={!draft.diagnostics.enabled} onCheckedChange={(enabled) => patchDiagnostics({ runtime_enabled: enabled })} />
                        <p className="text-xs text-muted-foreground">{t("runtimeLogRestartNotice")}</p>
                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("runtimeLogMaxSize")}</FieldTitle>
                            <FieldDescription>{t("runtimeLogMaxSizeDescription")}</FieldDescription>
                          </FieldContent>
                          <InputGroup className={compactInputGroupClass}>
                            <InputGroupInput
                              inputMode="decimal"
                              value={String(draft.diagnostics.runtime_max_size_bytes / 1024 / 1024).replace(/\.0$/, "")}
                              onChange={(event) => patchDiagnostics({ runtime_max_size_bytes: Math.max(1, Number(event.target.value || 1) * 1024 * 1024) })}
                            />
                            <InputGroupAddon align="inline-end">MB</InputGroupAddon>
                          </InputGroup>
                        </Field>
                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("runtimeLogRetentionDays")}</FieldTitle>
                            <FieldDescription>{t("unlimited")}</FieldDescription>
                          </FieldContent>
                          <InputGroup className={compactInputGroupClass}>
                            <InputGroupInput
                              inputMode="numeric"
                              placeholder={t("unlimitedPlaceholder")}
                              value={draft.diagnostics.runtime_retention_days ?? ""}
                              onChange={(event) => patchDiagnostics({ runtime_retention_days: event.target.value === "" ? null : Math.max(0, Number(event.target.value)) })}
                            />
                            <InputGroupAddon align="inline-end">{t("days")}</InputGroupAddon>
                          </InputGroup>
                        </Field>
                      </FieldGroup>
                    </section>
                  ) : null}

                  {section === "about" ? (
                    <section className="flex flex-col">
                      <SectionHead title={t("about")} description={t("settingsDescription")} />
                      <div className="divide-y">
                        <AboutRow label={t("version")} value={meta?.version ? `v${meta.version}` : "-"} />
                        <AboutRow label={t("github")} href="https://github.com/ip2a/mcpstore" value="github.com/ip2a/mcpstore" />
                        <AboutRow label={t("rustCrate")} href="https://crates.io/crates/mcpstore" value="crates.io/crates/mcpstore" />
                      </div>
                    </section>
                  ) : null}

                </DialogForm>
              ) : null}
            </div>
          </ScrollArea>
        </div>

        {draft ? (
          <DialogFormFooter
            className="shrink-0 border-t px-4 py-3 sm:px-5"
            onCancel={() => handleOpenChange(false)}
            submitDisabled={!draft}
            submitLabel={
              <>
                {!saving ? <SaveIcon data-icon="inline-start" /> : null}
                {t("save")}
              </>
            }
            submitButtonProps={{ form: "settings-form" }}
            submitting={saving}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

function SectionHead({ title, description, action }: { title: string; description?: string; action?: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b pb-3">
      <div className="min-w-0">
        <h3 className="text-base font-semibold">{title}</h3>
        {description ? <p className="mt-1 text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {action}
    </div>
  )
}

function ToggleField({ title, description, checked, disabled, onCheckedChange }: { title: string; description?: string; checked: boolean; disabled?: boolean; onCheckedChange: (checked: boolean) => void }) {
  return (
    <Field orientation="responsive">
      <FieldContent>
        <FieldTitle>{title}</FieldTitle>
        {description ? <FieldDescription>{description}</FieldDescription> : null}
      </FieldContent>
      <Switch checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} aria-label={title} />
    </Field>
  )
}

function AboutRow({ href, label, value }: { href?: string; label: string; value: string }) {
  return (
    <div className="flex min-w-0 items-baseline justify-between gap-4 py-3">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      {href ? (
        <a href={href} target="_blank" rel="noreferrer" className="min-w-0 truncate text-sm font-medium hover:underline">
          {value}
        </a>
      ) : (
        <span className="min-w-0 truncate text-sm font-medium">{value}</span>
      )}
    </div>
  )
}

function SettingsLoading({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground">
      <Spinner />
      {label}
    </div>
  )
}

function SettingsError({ message, onRetry }: { message: string; onRetry: () => void }) {
  const { t } = useI18n()

  return (
    <div className="flex flex-col gap-3 rounded-md border border-destructive/30 p-4 text-sm">
      <div className="flex items-start gap-2 text-destructive">
        <AlertCircleIcon className="mt-0.5 size-4" />
        <div>
          <p className="font-medium">{t("settingsUnavailable")}</p>
          <p className="mt-1 text-muted-foreground">{message}</p>
        </div>
      </div>
      <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
        <RefreshCwIcon data-icon="inline-start" />
        {t("retry")}
      </Button>
    </div>
  )
}

function OverviewSection({ draft }: { draft: SettingsDraft }) {
  const { t } = useI18n()
  const [statuses, setStatuses] = useState<Record<string, HostStatus>>({})

  useEffect(() => {
    let cancelled = false

    async function probeAll() {
      await Promise.all(
        draft.connections.map(async (connection) => {
          const status = await probeHost(connection.url)
          if (!cancelled) setStatuses((current) => ({ ...current, [connection.id]: status }))
        }),
      )
    }

    void probeAll()
    const timer = window.setInterval(() => void probeAll(), 5000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [draft.connections])

  return (
    <section className="flex flex-col gap-5">
      <SectionHead title={t("overview")} description={t("overviewDescription")} />
      <div className="flex flex-col gap-3">
        {draft.connections.map((connection) => {
          const status = statuses[connection.id]
          const isActive = connection.id === draft.activeConnectionId
          const online = status?.online === true

          return (
            <div
              key={connection.id}
              className={cn("rounded-md border p-4", isActive && "border-primary/40 bg-primary/5")}
            >
              <h4 className="truncate text-sm font-medium">{formatConnectionLabel(connection.url)}</h4>
              <div className="mt-2.5 flex min-w-0 items-center gap-2 overflow-hidden">
                <Badge variant="secondary" className="gap-1.5">
                  <span
                    className={cn(
                      "size-2 rounded-full",
                      status == null ? "bg-muted-foreground/40" : online ? "bg-emerald-500" : "bg-destructive",
                    )}
                  />
                  {status == null ? "…" : online ? t("online") : t("offline")}
                </Badge>
                <Badge variant="secondary">
                  <MonitorIcon />
                  {formatHostAddress(connection.url)}
                </Badge>
                <Badge variant="secondary">{status?.version ? `v${status.version}` : "-"}</Badge>
                {online && status?.latencyMs != null ? (
                  <Badge variant="secondary" className="tabular-nums">
                    {formatLatency(status.latencyMs)}
                  </Badge>
                ) : null}
              </div>
              {online && status?.store ? (
                <p className="mt-2 truncate text-xs text-muted-foreground">{status.store}</p>
              ) : null}
            </div>
          )
        })}
      </div>
    </section>
  )
}

function BackendFields({ draft, patchDraft }: { draft: SettingsDraft; patchDraft: (patch: Partial<SettingsDraft>) => void }) {
  return (
    <FieldGroup>
      <Field orientation="responsive">
        <FieldContent><FieldTitle>默认后端启动端口</FieldTitle><FieldDescription>保存到 app 配置，CLI 未指定 --port 时使用。</FieldDescription></FieldContent>
        <InputGroup className={compactInputGroupClass}><InputGroupInput inputMode="numeric" value={draft.server.port} onChange={(event) => patchDraft({ server: { ...draft.server, port: Math.max(1, Number(event.target.value || 1)) } })} /></InputGroup>
      </Field>
      <Field orientation="responsive">
        <FieldContent><FieldTitle>默认前端启动端口</FieldTitle><FieldDescription>内置 Web 和开发脚本使用的默认端口。</FieldDescription></FieldContent>
        <InputGroup className={compactInputGroupClass}><InputGroupInput inputMode="numeric" value={draft.server.web_port} onChange={(event) => patchDraft({ server: { ...draft.server, web_port: Math.max(1, Number(event.target.value || 1)) } })} /></InputGroup>
      </Field>
    </FieldGroup>
  )
}

function formatConnectionLabel(base: string): string {
  try {
    const url = base.startsWith("http://") || base.startsWith("https://") ? new URL(base) : new URL(base, window.location.origin)
    const host = url.port ? `${url.hostname}:${url.port}` : url.hostname
    return `HTTP (${host}${url.pathname !== "/" ? url.pathname : ""})`
  } catch {
    return `HTTP (${base})`
  }
}

function formatLatency(ms: number): string {
  if (ms < 1) return `${Math.max(1, Math.round(ms * 1000))}μs`
  return `${Math.round(ms)}ms`
}

async function measureConnectionLatency(url: string): Promise<number | null> {
  const start = performance.now()
  try {
    const response = await fetch(`${url.replace(/\/$/, "")}/health`)
    return response.ok ? performance.now() - start : null
  } catch {
    return null
  }
}

type HostStatus = {
  online: boolean
  latencyMs: number | null
  version: string | null
  store: string | null
}

async function probeHost(url: string): Promise<HostStatus> {
  const base = url.replace(/\/$/, "")
  const start = performance.now()
  try {
    const [healthResponse, version] = await Promise.all([
      fetch(`${base}/health`),
      fetchHostVersion(base),
    ])
    const latencyMs = performance.now() - start
    if (!healthResponse.ok) return { online: false, latencyMs: null, version, store: null }
    const body = (await healthResponse.json().catch(() => null)) as { store?: unknown } | null
    return {
      online: true,
      latencyMs,
      version,
      store: typeof body?.store === "string" ? body.store : null,
    }
  } catch {
    return { online: false, latencyMs: null, version: null, store: null }
  }
}

async function fetchHostVersion(base: string): Promise<string | null> {
  try {
    const response = await fetch(`${base}/v1/meta`)
    if (!response.ok) return null
    const body = await response.json()
    const data = body && typeof body === "object" && "data" in body ? body.data : body
    const version = (data as { version?: unknown } | null)?.version
    return typeof version === "string" ? version : null
  } catch {
    return null
  }
}

function formatHostAddress(url: string): string {
  try {
    const parsed = url.startsWith("http://") || url.startsWith("https://") ? new URL(url) : new URL(url, window.location.origin)
    return parsed.port ? `${parsed.hostname}:${parsed.port}` : parsed.hostname
  } catch {
    return url
  }
}

function composeConnectionUrl(host: string, port: string, secure: boolean, path: string): string {
  const trimmedHost = host.trim()
  if (!trimmedHost) return ""
  const trimmedPort = port.trim()
  let trimmedPath = path.trim()
  if (trimmedPath && !trimmedPath.startsWith("/")) trimmedPath = `/${trimmedPath}`
  return `${secure ? "https" : "http"}://${trimmedHost}${trimmedPort ? `:${trimmedPort}` : ""}${trimmedPath}`
}

function formatFullUrlBody(host: string, port: string, path: string): string {
  const trimmedHost = host.trim()
  if (!trimmedHost) return ""
  const trimmedPort = port.trim()
  let trimmedPath = path.trim()
  if (trimmedPath && !trimmedPath.startsWith("/")) trimmedPath = `/${trimmedPath}`
  return `${trimmedHost}${trimmedPort ? `:${trimmedPort}` : ""}${trimmedPath}`
}

function composeFullUrlFromBody(body: string, secure: boolean): string {
  const trimmed = body.trim().replace(/^https?:\/\//i, "")
  if (!trimmed) return ""
  return `${secure ? "https" : "http"}://${trimmed}`
}

function parseFullUrlBodyInput(value: string): { body: string; secure?: boolean } {
  const trimmed = value.trim()
  const match = trimmed.match(/^(https?):\/\/(.*)$/i)
  if (match) {
    return { body: match[2], secure: match[1].toLowerCase() === "https" }
  }
  return { body: trimmed.replace(/^https?:\/\//i, "") }
}

function parseConnectionUrl(url: string): { host: string; port: string; secure: boolean; path: string } | null {
  const trimmed = url.trim()
  if (!trimmed) return null
  try {
    const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`
    const parsed = new URL(withScheme)
    if (!parsed.hostname) return null
    return {
      host: parsed.hostname,
      port: parsed.port,
      secure: parsed.protocol === "https:",
      path: parsed.pathname === "/" ? "" : `${parsed.pathname}${parsed.search}`,
    }
  } catch {
    return null
  }
}

function ConnectionSection({ draft, patchDraft }: { draft: SettingsDraft; patchDraft: (patch: Partial<SettingsDraft>) => void }) {
  const { t } = useI18n()
  const [addingOpen, setAddingOpen] = useState(false)
  const [pendingHost, setPendingHost] = useState("")
  const [pendingPort, setPendingPort] = useState("")
  const [pendingSecure, setPendingSecure] = useState(false)
  const [pendingPath, setPendingPath] = useState("")
  const [pendingFullUrlMode, setPendingFullUrlMode] = useState(false)
  const [pendingFullUrlBody, setPendingFullUrlBody] = useState("")
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editUrl, setEditUrl] = useState("")
  const [latencies, setLatencies] = useState<Record<string, number | null>>({})
  const [checking, setChecking] = useState<Record<string, boolean>>({})

  async function refreshLatency(id: string, url: string, options?: { notify?: boolean }) {
    setChecking((current) => ({ ...current, [id]: true }))
    const latency = await measureConnectionLatency(url)
    setLatencies((current) => ({ ...current, [id]: latency }))
    setChecking((current) => ({ ...current, [id]: false }))
    if (options?.notify) {
      if (latency != null) toast.success(t("connectionRefreshOk", { latency: formatLatency(latency) }))
      else toast.error(t("connectionRefreshFailed"))
    }
  }

  useEffect(() => {
    let cancelled = false

    async function measureAll() {
      await Promise.all(
        draft.connections.map(async (connection) => {
          if (cancelled) return
          await refreshLatency(connection.id, connection.url)
        }),
      )
    }

    void measureAll()
    const timer = window.setInterval(() => void measureAll(), 5000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [draft.connections])

  const pendingUrl = composeConnectionUrl(pendingHost, pendingPort, pendingSecure, pendingPath)
  const pendingFullUrl = composeFullUrlFromBody(pendingFullUrlBody, pendingSecure)
  const parsedRawUrl = pendingFullUrlMode ? parseConnectionUrl(pendingFullUrl) : null
  const pendingValid = pendingFullUrlMode ? parsedRawUrl != null : pendingUrl !== ""

  function resetPending() {
    setPendingHost("")
    setPendingPort("")
    setPendingSecure(false)
    setPendingPath("")
    setPendingFullUrlMode(false)
    setPendingFullUrlBody("")
  }

  function addConnection() {
    const url = pendingFullUrlMode ? pendingFullUrl : pendingUrl
    if (!pendingValid || !url) return
    const connection: ConnectionDraft = { id: crypto.randomUUID(), url }
    patchDraft({ connections: [...draft.connections, connection] })
    resetPending()
    setAddingOpen(false)
  }

  function setFullUrlMode(enabled: boolean) {
    if (!enabled) {
      const parsed = parseConnectionUrl(pendingFullUrl)
      if (parsed) {
        setPendingHost(parsed.host)
        setPendingPort(parsed.port)
        setPendingSecure(parsed.secure)
        setPendingPath(parsed.path)
      }
      setPendingFullUrlMode(false)
      return
    }
    const parsed = parseConnectionUrl(pendingUrl)
    setPendingFullUrlBody(parsed ? formatFullUrlBody(parsed.host, parsed.port, parsed.path) : "")
    if (parsed) setPendingSecure(parsed.secure)
    setPendingFullUrlMode(true)
  }

  function saveEdit() {
    if (!editingId) return
    const url = editUrl.trim()
    if (!url) return
    patchDraft({
      connections: draft.connections.map((connection) =>
        connection.id === editingId ? { ...connection, url } : connection,
      ),
    })
    setEditingId(null)
    setEditUrl("")
  }

  function removeConnection(id: string) {
    if (draft.connections.length <= 1) return
    const next = draft.connections.filter((connection) => connection.id !== id)
    patchDraft({
      connections: next,
      activeConnectionId: draft.activeConnectionId === id ? next[0].id : draft.activeConnectionId,
    })
    if (editingId === id) {
      setEditingId(null)
      setEditUrl("")
    }
    toast.success(t("connectionDeleted"))
  }

  function toggleEdit(connection: ConnectionDraft) {
    if (editingId === connection.id) {
      setEditingId(null)
      setEditUrl("")
      return
    }
    startEdit(connection)
  }

  function startEdit(connection: ConnectionDraft) {
    setEditingId(connection.id)
    setEditUrl(connection.url)
    setAddingOpen(false)
  }

  return (
    <section className="flex flex-col gap-5">
      <SectionHead
        title={t("connection")}
        description={t("connectionDescription")}
        action={
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={() => {
              setEditingId(null)
              setEditUrl("")
              resetPending()
              setAddingOpen(true)
            }}
          >
            <PlusIcon data-icon="inline-start" />
            {t("addHost")}
          </Button>
        }
      />

      <div className="flex flex-col gap-2">
        <p className="text-xs text-muted-foreground">{t("connection")}</p>
        <div className="flex flex-col gap-2">
          {draft.connections.map((connection) => {
            const isActive = connection.id === draft.activeConnectionId
            const isEditing = editingId === connection.id
            const canDelete = draft.connections.length > 1
            const isChecking = checking[connection.id] === true

            return (
              <div key={connection.id} className="flex flex-col gap-2">
                <div
                  className={cn(
                    "flex items-center justify-between gap-3 rounded-md border px-3 py-2.5",
                    isActive && "border-primary/40 bg-primary/5",
                  )}
                >
                  <button
                    type="button"
                    className="min-w-0 flex-1 truncate text-left text-sm"
                    onClick={() => patchDraft({ activeConnectionId: connection.id })}
                  >
                    {formatConnectionLabel(connection.url)}
                  </button>
                  <div className="flex shrink-0 items-center gap-2">
                    <span className="text-xs text-muted-foreground tabular-nums">
                      {isChecking && latencies[connection.id] == null
                        ? "…"
                        : latencies[connection.id] != null
                          ? formatLatency(latencies[connection.id]!)
                          : "-"}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("refresh")}
                      disabled={isChecking}
                      onClick={() => void refreshLatency(connection.id, connection.url, { notify: true })}
                    >
                      <RefreshCwIcon className={cn("size-3.5", isChecking && "animate-spin")} />
                    </Button>
                    <Button
                      type="button"
                      variant={isEditing ? "secondary" : "ghost"}
                      size="icon-sm"
                      aria-label={t("edit")}
                      aria-pressed={isEditing}
                      onClick={() => toggleEdit(connection)}
                    >
                      <PencilIcon className="size-3.5" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("delete")}
                      disabled={!canDelete}
                      title={!canDelete ? t("connectionDeleteLastHint") : undefined}
                      onClick={() => removeConnection(connection.id)}
                    >
                      <Trash2Icon className={cn("size-3.5", canDelete ? "text-destructive" : "text-muted-foreground")} />
                    </Button>
                  </div>
                </div>

                {isEditing ? (
                  <Collapsible open onOpenChange={(open) => !open && setEditingId(null)}>
                    <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-out data-[state=open]:animate-in">
                      <div className="rounded-md border px-3 py-3">
                        <p className="mb-2 text-xs text-muted-foreground">{t("edit")}</p>
                        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
                          <InputGroup className="min-w-0 flex-1">
                            <InputGroupInput
                              value={editUrl}
                              onChange={(event) => setEditUrl(event.target.value)}
                              placeholder={t("coreBackendUrlPlaceholder")}
                            />
                          </InputGroup>
                          <div className="flex gap-2">
                            <Button type="button" size="sm" onClick={saveEdit}>
                              {t("save")}
                            </Button>
                            <Button type="button" size="sm" variant="outline" onClick={() => setEditingId(null)}>
                              {t("cancel")}
                            </Button>
                          </div>
                        </div>
                      </div>
                    </CollapsibleContent>
                  </Collapsible>
                ) : null}
              </div>
            )
          })}
        </div>
      </div>

      {addingOpen ? (
        <div className="flex flex-col gap-4 rounded-md border p-4">
          {pendingFullUrlMode ? (
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="connection-full-url">{t("fullUrl")}</Label>
              <div className="flex items-center gap-2">
                <span className="shrink-0 font-mono text-sm text-muted-foreground">{pendingSecure ? "https://" : "http://"}</span>
                <InputGroup className="min-w-0 flex-1">
                  <InputGroupInput
                    id="connection-full-url"
                    autoFocus
                    value={pendingFullUrlBody}
                    onChange={(event) => {
                      const next = parseFullUrlBodyInput(event.target.value)
                      setPendingFullUrlBody(next.body)
                      if (next.secure != null) setPendingSecure(next.secure)
                    }}
                    placeholder="127.0.0.1:1820/api"
                    className={cn(pendingFullUrlBody.trim() && !parsedRawUrl && "border-destructive/50")}
                  />
                </InputGroup>
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-4 sm:flex-row">
              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <Label htmlFor="connection-host">{t("host")}</Label>
                <div className="flex items-center gap-2">
                  <span className="shrink-0 font-mono text-sm text-muted-foreground">{pendingSecure ? "https://" : "http://"}</span>
                  <InputGroup className="min-w-0 flex-1">
                    <InputGroupInput
                      id="connection-host"
                      autoFocus
                      value={pendingHost}
                      onChange={(event) => setPendingHost(event.target.value)}
                      placeholder="127.0.0.1"
                    />
                  </InputGroup>
                </div>
              </div>
              <div className="flex w-full flex-col gap-1.5 sm:w-32">
                <Label htmlFor="connection-port">{t("port")}</Label>
                <InputGroup>
                  <InputGroupInput
                    id="connection-port"
                    inputMode="numeric"
                    value={pendingPort}
                    onChange={(event) => setPendingPort(event.target.value.replace(/\D/g, ""))}
                    placeholder="1820"
                  />
                </InputGroup>
              </div>
            </div>
          )}

          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <label className="flex items-center gap-2 text-sm">
              <Switch checked={pendingSecure} onCheckedChange={setPendingSecure} aria-label={t("useHttps")} />
              {t("useHttps")}
            </label>
            <label className="flex items-center gap-2 text-sm">
              <Switch checked={pendingFullUrlMode} onCheckedChange={setFullUrlMode} aria-label={t("useFullUrl")} />
              {t("useFullUrl")}
            </label>
            <div className="ml-auto flex shrink-0 items-center gap-2">
              <Button type="button" variant="ghost" size="icon-sm" aria-label={t("delete")} onClick={() => { resetPending(); setAddingOpen(false) }}>
                <Trash2Icon className="size-3.5 text-destructive" />
              </Button>
              <Button type="button" variant="ghost" size="icon-sm" aria-label={t("add")} disabled={!pendingValid} onClick={addConnection}>
                <CheckIcon className="size-3.5" />
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  )
}
