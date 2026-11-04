import { useEffect, useMemo, useState, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ActivityIcon, FileTextIcon, InfoIcon, MonitorIcon, NetworkIcon, PencilIcon, PlusIcon, RefreshCwIcon, SaveIcon, ServerIcon, SlidersHorizontalIcon, Trash2Icon, type LucideIcon } from "lucide-react"
import { toast } from "sonner"

import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { McpstoreAsciiBanner } from "@/components/shared/mcpstore-ascii-banner"
import { MarkIcon } from "@/components/shared/mark-icon"
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Field, FieldContent, FieldDescription, FieldGroup, FieldTitle } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { hostsToList, payloadFromDraft, sections, settingsDraft, type SectionId, type SettingsDraft } from "@/features/settings/model"
import { formatReleaseVersion, isDesktopShell, UpdateButton, useAppUpdater, useAppVersionLabel } from "@/features/settings/updater"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { getHostStatus, type HostStatus as ApiHostStatus, type UiLanguage } from "@/lib/api"
import { syncApiBaseFromHosts } from "@/lib/api/backend"
import { validateHostsConfig } from "@/lib/api/hosts"
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

export function SettingsView() {
  const { setLanguageOverride, t } = useI18n()
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<SectionId>("overview")
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const metaQuery = useSettingsMetaQuery(true)
  const settingsMutation = useUpdateSettingsMutation()
  const { phase, progress, version: updateAvailable, checkForUpdate, restartToUpdate } = useAppUpdater()
  const appVersion = useAppVersionLabel()
  const meta = metaQuery.data
  const loading = metaQuery.isFetching && !draft
  const error = metaQuery.error
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : t("settingsUnavailable")
  const saving = settingsMutation.isPending

  const configFile = meta?.config_file
  const configContent = useMemo(() => configFile?.content || "", [configFile?.content])

  const headerLine = useMemo(() => {
    if (!draft) return t("settingsDescription")
    const parts = [draft.hosts.active]
    if (configFile?.path) parts.push(configFile.path)
    return parts.join(" · ")
  }, [configFile?.path, draft, t])

  useEffect(() => {
    if (meta?.settings) setDraft(settingsDraft(meta.settings))
  }, [meta])

  function patchDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => (current ? { ...current, ...patch } : current))
  }

  function patchDiagnostics(patch: Partial<SettingsDraft["diagnostics"]>) {
    setDraft((current) => (current ? { ...current, diagnostics: { ...current.diagnostics, ...patch } } : current))
  }

  async function handleSave() {
    if (!draft) return
    const validationError = validateHostsConfig(draft.hosts)
    if (validationError) {
      toast.error(validationError)
      return
    }
    try {
      await settingsMutation.mutateAsync(payloadFromDraft(draft))
      setLanguageOverride(draft.language)
      syncApiBaseFromHosts(draft.hosts)
      toast.success(t("saved"))
      await queryClient.invalidateQueries({ queryKey: queryKeys.meta })
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("saveFailed"))
    }
  }

  const stateGuard =
    error && !draft ? (
      <PageError
        title={t("settingsUnavailable")}
        message={errorMessage}
        onRefresh={() => void metaQuery.refetch()}
      />
    ) : loading && !draft ? (
      <PageSkeleton />
    ) : null

  return (
    <>
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="text-lg font-semibold">{t("settings")}</h1>
        <p className="mt-1 truncate font-mono text-sm text-muted-foreground" title={headerLine}>
          {headerLine}
        </p>
      </div>

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col overflow-hidden">
        {stateGuard ?? (
          draft ? (
            <Tabs
              value={activeTab}
              onValueChange={(value) => setActiveTab(value as SectionId)}
              className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
            >
              <div className="flex min-w-0 shrink-0 items-center gap-3 pb-2">
                <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start gap-1">
                  {sections.map((item) => {
                    const Icon = sectionIcons[item.id]
                    const label = t(item.labelKey)
                    return (
                      <TabsTrigger key={item.id} value={item.id} title={label}>
                        <Icon />
                        {label}
                      </TabsTrigger>
                    )
                  })}
                </TabsList>
                <div className="ml-auto flex shrink-0 gap-2">
                  <Button size="sm" variant="outline" onClick={() => void metaQuery.refetch()} disabled={metaQuery.isFetching}>
                    <RefreshCwIcon data-icon="inline-start" />
                    {t("refresh")}
                  </Button>
                  <Button size="sm" onClick={() => void handleSave()} disabled={saving}>
                    {saving ? <Spinner /> : <SaveIcon data-icon="inline-start" />}
                    {t("save")}
                  </Button>
                </div>
              </div>

              <TabsContent value="overview" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
                  <OverviewSection draft={draft} />
                </ScrollPane>
              </TabsContent>

              <TabsContent value="general" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
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
                </ScrollPane>
              </TabsContent>

              <TabsContent value="connection" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
                  <ConnectionSection draft={draft} patchDraft={patchDraft} />
                </ScrollPane>
              </TabsContent>

              <TabsContent value="diagnostics" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
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
                </ScrollPane>
              </TabsContent>

              <TabsContent value="config" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
                  <section className="flex flex-col gap-4">
                    <SectionHead title={t("configFile")} description={configFile?.path || t("configFileMissing")} />
                    <InputGroup>
                      <InputGroupTextarea className="min-h-80 font-mono text-xs" readOnly value={configContent} />
                    </InputGroup>
                  </section>
                </ScrollPane>
              </TabsContent>

              <TabsContent value="about" className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden">
                <ScrollPane className="flex-1">
                  <section className="flex flex-col gap-5">
                    <McpstoreAsciiBanner />
                    <SectionHead
                      title={t("about")}
                      description={t("settingsDescription")}
                      action={<MarkIcon className="size-8" />}
                    />
                    <div className="divide-y">
                      <AboutRow label={t("appVersion")} value={formatReleaseVersion(appVersion)} />
                      <AboutRow label={t("sdkVersion")} value={formatReleaseVersion(meta?.version)} />
                      {isDesktopShell ? (
                        <AboutRow
                          label={updateAvailable ? t("updateAvailable", { version: `v${updateAvailable}` }) : t("checkUpdates")}
                          action={
                            <UpdateButton
                              phase={phase}
                              progress={progress}
                              onCheck={() => void checkForUpdate()}
                              onRestart={restartToUpdate}
                            />
                          }
                        />
                      ) : null}
                      <AboutRow label={t("github")} href="https://github.com/ip2a/mcpstore" value="https://github.com/ip2a/mcpstore" />
                      <AboutRow label={t("rustCrate")} href="https://crates.io/crates/mcpstore" value="https://crates.io/crates/mcpstore" />
                      <AboutRow label={t("pythonPackage")} href="https://pypi.org/project/mcpstore/" value="https://pypi.org/project/mcpstore/" />
                      <AboutRow label={t("npmPackage")} href="https://www.npmjs.com/package/@ip2a/mcpstore" value="https://www.npmjs.com/package/@ip2a/mcpstore" />
                    </div>
                  </section>
                </ScrollPane>
              </TabsContent>
            </Tabs>
          ) : (
            <PageSkeleton />
          )
        )}
      </PanelCard>
    </>
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

function AboutRow({ href, label, value, action }: { href?: string; label: string; value?: string; action?: ReactNode }) {
  return (
    <div className="flex min-w-0 items-center justify-between gap-4 py-3">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      {action ? (
        action
      ) : href ? (
        <a href={href} target="_blank" rel="noreferrer" className="min-w-0 truncate text-sm font-medium hover:underline">
          {value}
        </a>
      ) : (
        <span className="min-w-0 truncate text-sm font-medium">{value}</span>
      )}
    </div>
  )
}

function OverviewSection({ draft }: { draft: SettingsDraft }) {
  const { t } = useI18n()
  const [statuses, setStatuses] = useState<Record<string, HostStatus>>({})
  const hosts = useMemo(() => hostsToList(draft.hosts), [draft.hosts])

  useEffect(() => {
    let cancelled = false

    async function probeAll() {
      await Promise.all(
        hosts.map(async (host) => {
          const status = await probeHost(host.name)
          if (!cancelled) setStatuses((current) => ({ ...current, [host.name]: status }))
        }),
      )
    }

    void probeAll()
    const timer = window.setInterval(() => void probeAll(), 5000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [hosts])

  return (
    <section className="flex flex-col gap-5">
      <SectionHead title={t("overview")} description={t("overviewDescription")} />
      <div className="flex flex-col gap-3">
        {hosts.map((host) => {
          const status = statuses[host.name]
          const isActive = host.name === draft.hosts.active
          const online = status?.online === true

          return (
            <div
              key={host.name}
              className={cn("rounded-md border p-4", isActive && "border-primary/40 bg-primary/5")}
            >
              <h4 className="truncate text-sm font-medium">{host.name}</h4>
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
                  {formatHostAddress(host.urls[0] ?? "")}
                </Badge>
                {host.urls.length > 1 ? (
                  <Badge variant="secondary" title={host.urls.join("\n")}>
                    +{host.urls.length - 1}
                  </Badge>
                ) : null}
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
  const { t } = useI18n()
  return (
    <FieldGroup>
      <Field orientation="responsive">
        <FieldContent>
          <FieldTitle>{t("defaultBackendPort")}</FieldTitle>
          <FieldDescription>{t("defaultBackendPortDescription")}</FieldDescription>
        </FieldContent>
        <InputGroup className={compactInputGroupClass}><InputGroupInput inputMode="numeric" value={draft.api.port} onChange={(event) => patchDraft({ api: { ...draft.api, port: Math.max(1, Number(event.target.value || 1)) } })} /></InputGroup>
      </Field>
      <Field orientation="responsive">
        <FieldContent>
          <FieldTitle>{t("defaultFrontendPort")}</FieldTitle>
          <FieldDescription>{t("defaultFrontendPortDescription")}</FieldDescription>
        </FieldContent>
        <InputGroup className={compactInputGroupClass}><InputGroupInput inputMode="numeric" value={draft.web.port} onChange={(event) => patchDraft({ web: { ...draft.web, port: Math.max(1, Number(event.target.value || 1)) } })} /></InputGroup>
      </Field>
    </FieldGroup>
  )
}

function formatLatency(ms: number): string {
  if (ms < 1) return `${Math.max(1, Math.round(ms * 1000))}μs`
  return `${Math.round(ms)}ms`
}

type HostStatus = {
  online: boolean
  latencyMs: number | null
  version: string | null
  store: string | null
}

function toHostStatus(status: ApiHostStatus): HostStatus {
  return {
    online: status.online,
    latencyMs: status.latency_ms,
    version: status.version,
    store: status.store,
  }
}

async function probeHost(name: string): Promise<HostStatus> {
  try {
    return toHostStatus(await getHostStatus(name))
  } catch {
    return { online: false, latencyMs: null, version: null, store: null }
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
  const hosts = useMemo(() => hostsToList(draft.hosts), [draft.hosts])
  const [addingOpen, setAddingOpen] = useState(false)
  const [pendingName, setPendingName] = useState("")
  const [pendingHost, setPendingHost] = useState("")
  const [pendingPort, setPendingPort] = useState("")
  const [pendingSecure, setPendingSecure] = useState(false)
  const [pendingPath, setPendingPath] = useState("")
  const [pendingFullUrlMode, setPendingFullUrlMode] = useState(false)
  const [pendingFullUrlBody, setPendingFullUrlBody] = useState("")
  const [editingName, setEditingName] = useState<string | null>(null)
  const [editName, setEditName] = useState("")
  const [editUrls, setEditUrls] = useState("")
  const [latencies, setLatencies] = useState<Record<string, number | null>>({})
  const [checking, setChecking] = useState<Record<string, boolean>>({})

  function patchHosts(nextEntries: Record<string, { urls: string[] }>, active = draft.hosts.active) {
    patchDraft({ hosts: { active, entries: nextEntries } })
  }

  async function refreshLatency(name: string, options?: { notify?: boolean }) {
    setChecking((current) => ({ ...current, [name]: true }))
    const status = await probeHost(name)
    const latency = status.online ? status.latencyMs : null
    setLatencies((current) => ({ ...current, [name]: latency }))
    setChecking((current) => ({ ...current, [name]: false }))
    if (options?.notify) {
      if (latency != null) toast.success(t("connectionRefreshOk", { latency: formatLatency(latency) }))
      else toast.error(t("connectionRefreshFailed"))
    }
  }

  useEffect(() => {
    let cancelled = false

    async function measureAll() {
      await Promise.all(
        hosts.map(async (host) => {
          if (cancelled) return
          await refreshLatency(host.name)
        }),
      )
    }

    void measureAll()
    const timer = window.setInterval(() => void measureAll(), 5000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [hosts])

  const pendingUrl = composeConnectionUrl(pendingHost, pendingPort, pendingSecure, pendingPath)
  const pendingFullUrl = composeFullUrlFromBody(pendingFullUrlBody, pendingSecure)
  const parsedRawUrl = pendingFullUrlMode ? parseConnectionUrl(pendingFullUrl) : null
  const pendingUrlValue = pendingFullUrlMode ? pendingFullUrl : pendingUrl
  const pendingValid =
    pendingName.trim() !== "" &&
    !Object.prototype.hasOwnProperty.call(draft.hosts.entries, pendingName.trim()) &&
    (pendingFullUrlMode ? parsedRawUrl != null : pendingUrl !== "")

  function resetPending() {
    setPendingName("")
    setPendingHost("")
    setPendingPort("")
    setPendingSecure(false)
    setPendingPath("")
    setPendingFullUrlMode(false)
    setPendingFullUrlBody("")
  }

  function handleAddDialogOpenChange(open: boolean) {
    if (!open) resetPending()
    setAddingOpen(open)
  }

  function addHost() {
    const name = pendingName.trim()
    const url = pendingUrlValue.trim()
    if (!pendingValid || !name || !url) return
    patchHosts({
      ...draft.hosts.entries,
      [name]: { urls: [url] },
    })
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

  /** One URL per line; order is the failover preference. */
  function parseEditUrls(): string[] {
    return editUrls
      .split("\n")
      .map((url) => url.trim())
      .filter((url) => url !== "")
  }

  function saveEdit() {
    if (!editingName) return
    const name = editName.trim()
    const urls = parseEditUrls()
    if (!name || urls.length === 0) return
    if (name !== editingName && Object.prototype.hasOwnProperty.call(draft.hosts.entries, name)) {
      toast.error(t("saveFailed"))
      return
    }
    const entries = { ...draft.hosts.entries }
    delete entries[editingName]
    entries[name] = { urls }
    const active = draft.hosts.active === editingName ? name : draft.hosts.active
    patchHosts(entries, active)
    setEditingName(null)
    setEditName("")
    setEditUrls("")
  }

  function removeHost(name: string) {
    if (hosts.length <= 1) return
    const entries = { ...draft.hosts.entries }
    delete entries[name]
    const remaining = Object.keys(entries)
    const active = draft.hosts.active === name ? remaining[0]! : draft.hosts.active
    patchHosts(entries, active)
    if (editingName === name) {
      setEditingName(null)
      setEditName("")
      setEditUrls("")
    }
    toast.success(t("connectionDeleted"))
  }

  function toggleEdit(host: { name: string; urls: string[] }) {
    if (editingName === host.name) {
      setEditingName(null)
      setEditName("")
      setEditUrls("")
      return
    }
    setEditingName(host.name)
    setEditName(host.name)
    setEditUrls(host.urls.join("\n"))
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
              setEditingName(null)
              setEditName("")
              setEditUrls("")
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
          {hosts.map((host) => {
            const isActive = host.name === draft.hosts.active
            const isEditing = editingName === host.name
            const canDelete = hosts.length > 1
            const isChecking = checking[host.name] === true

            return (
              <div key={host.name} className="flex flex-col gap-2">
                <div
                  className={cn(
                    "flex items-center justify-between gap-3 rounded-md border px-3 py-2.5",
                    isActive && "border-primary/40 bg-primary/5",
                  )}
                >
                  <button
                    type="button"
                    className="min-w-0 flex-1 truncate text-left"
                    onClick={() => patchDraft({ hosts: { ...draft.hosts, active: host.name } })}
                  >
                    <span className="block truncate text-sm font-medium">{host.name}</span>
                    <span className="block truncate text-xs text-muted-foreground">
                      {host.urls.length > 1 ? `${formatHostAddress(host.urls[0] ?? "")} +${host.urls.length - 1}` : host.urls[0]}
                    </span>
                  </button>
                  <div className="flex shrink-0 items-center gap-2">
                    <span className="text-xs text-muted-foreground tabular-nums">
                      {isChecking && latencies[host.name] == null
                        ? "…"
                        : latencies[host.name] != null
                          ? formatLatency(latencies[host.name]!)
                          : "-"}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("refresh")}
                      disabled={isChecking}
                      onClick={() => void refreshLatency(host.name, { notify: true })}
                    >
                      <RefreshCwIcon className={cn("size-3.5", isChecking && "animate-spin")} />
                    </Button>
                    <Button
                      type="button"
                      variant={isEditing ? "secondary" : "ghost"}
                      size="icon-sm"
                      aria-label={t("edit")}
                      aria-pressed={isEditing}
                      onClick={() => toggleEdit(host)}
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
                      onClick={() => removeHost(host.name)}
                    >
                      <Trash2Icon className={cn("size-3.5", canDelete ? "text-destructive" : "text-muted-foreground")} />
                    </Button>
                  </div>
                </div>

                {isEditing ? (
                  <Collapsible open onOpenChange={(open) => !open && setEditingName(null)}>
                    <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-out data-[state=open]:animate-in">
                      <div className="rounded-md border px-3 py-3">
                        <p className="mb-2 text-xs text-muted-foreground">{t("edit")}</p>
                        <div className="flex flex-col gap-3">
                          <div className="flex flex-col gap-1.5">
                            <Label htmlFor={`host-name-${host.name}`}>{t("host")}</Label>
                            <InputGroup>
                              <InputGroupInput
                                id={`host-name-${host.name}`}
                                value={editName}
                                onChange={(event) => setEditName(event.target.value)}
                                placeholder={t("host")}
                              />
                            </InputGroup>
                          </div>
                          <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
                            <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                              <Label htmlFor={`host-url-${host.name}`}>{t("coreBackendUrlLabel")}</Label>
                              <InputGroup>
                                <InputGroupTextarea
                                  id={`host-url-${host.name}`}
                                  className="min-h-20 font-mono text-xs"
                                  value={editUrls}
                                  onChange={(event) => setEditUrls(event.target.value)}
                                  placeholder={t("coreBackendUrlPlaceholder")}
                                />
                              </InputGroup>
                              <p className="text-xs text-muted-foreground">{t("coreBackendUrlHint")}</p>
                            </div>
                            <div className="flex gap-2">
                              <Button type="button" size="sm" onClick={saveEdit}>
                                {t("save")}
                              </Button>
                              <Button type="button" size="sm" variant="outline" onClick={() => setEditingName(null)}>
                                {t("cancel")}
                              </Button>
                            </div>
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

      <Dialog open={addingOpen} onOpenChange={handleAddDialogOpenChange}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("addHost")}</DialogTitle>
            <DialogDescription>{t("connectionDescription")}</DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="pending-host-name">{t("host")}</Label>
              <InputGroup>
                <InputGroupInput
                  id="pending-host-name"
                  autoFocus
                  value={pendingName}
                  onChange={(event) => setPendingName(event.target.value)}
                  placeholder={t("host")}
                  className={cn(
                    pendingName.trim() &&
                      Object.prototype.hasOwnProperty.call(draft.hosts.entries, pendingName.trim()) &&
                      "border-destructive/50",
                  )}
                />
              </InputGroup>
            </div>

            {pendingFullUrlMode ? (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="connection-full-url">{t("fullUrl")}</Label>
                <div className="flex items-center gap-2">
                  <span className="shrink-0 font-mono text-sm text-muted-foreground">{pendingSecure ? "https://" : "http://"}</span>
                  <InputGroup className="min-w-0 flex-1">
                    <InputGroupInput
                      id="connection-full-url"
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
                  <Label htmlFor="connection-host">{t("coreBackendUrlLabel")}</Label>
                  <div className="flex items-center gap-2">
                    <span className="shrink-0 font-mono text-sm text-muted-foreground">{pendingSecure ? "https://" : "http://"}</span>
                    <InputGroup className="min-w-0 flex-1">
                      <InputGroupInput
                        id="connection-host"
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
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => handleAddDialogOpenChange(false)}>
              {t("cancel")}
            </Button>
            <Button type="button" disabled={!pendingValid} onClick={addHost}>
              <PlusIcon data-icon="inline-start" />
              {t("addHost")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  )
}
