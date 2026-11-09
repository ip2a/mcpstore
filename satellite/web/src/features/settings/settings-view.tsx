import { useEffect, useMemo, useState, type ReactNode } from "react"

import { useQueryClient } from "@tanstack/react-query"
import {
  DatabaseIcon,
  MonitorIcon,
  PencilIcon,
  PlusIcon,
  SaveIcon,
  Trash2Icon,
} from "lucide-react"
import { toast } from "sonner"

import { PanelCard } from "@/components/shared/panel-card"
import {
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { McpstoreAsciiBanner } from "@/components/shared/mcpstore-ascii-banner"
import { MarkIcon } from "@/components/shared/mark-icon"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupTextarea,
} from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import {
  hostsToList,
  payloadFromDraft,
  sections,
  settingsDraft,
  type SectionId,
  type SettingsDraft,
} from "@/features/settings/model"
import {
  formatReleaseVersion,
  isDesktopShell,
  UpdateButton,
  useAppUpdater,
  useAppVersionLabel,
} from "@/features/settings/updater"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { useCacheHealthQuery } from "@/features/cache/queries"
import { getHostStatus, type HostStatus as ApiHostStatus, type UiLanguage } from "@/lib/api"
import { syncApiBaseFromHosts } from "@/lib/api/backend"
import { validateHostsConfig } from "@/lib/api/hosts"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"
import { queryKeys } from "@/lib/query-keys"
import { useUiStore } from "@/stores/ui-store"

const compactInputGroupClass = "w-20 shrink-0"

export function SettingsView() {
  const { setLanguageOverride, t } = useI18n()
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<SectionId>("connection")
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const metaQuery = useSettingsMetaQuery(true)
  const settingsMutation = useUpdateSettingsMutation()
  const {
    phase,
    progress,
    version: updateAvailable,
    checkForUpdate,
    restartToUpdate,
  } = useAppUpdater()
  const appVersion = useAppVersionLabel()
  const meta = metaQuery.data
  const loading = metaQuery.isFetching && !draft
  const error = metaQuery.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("settingsUnavailable")
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
    setDraft((current) =>
      current ? { ...current, diagnostics: { ...current.diagnostics, ...patch } } : current,
    )
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

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        {stateGuard ? (
          <div className="p-4">{stateGuard}</div>
        ) : draft ? (
            <Tabs
              value={activeTab}
              onValueChange={(value) => setActiveTab(value as SectionId)}
              className={WORKSPACE_TABS_CLASSNAME}
            >
              <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
                <WorkspaceTabsList value={activeTab} className="gap-1">
                  {sections.map((item) => {
                    const label = t(item.labelKey)
                    return (
                      <TabsTrigger key={item.id} value={item.id} title={label}>
                        {label}
                      </TabsTrigger>
                    )
                  })}
                </WorkspaceTabsList>
                <div className="ml-auto flex h-9 shrink-0 items-center gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void metaQuery.refetch()}
                    disabled={metaQuery.isFetching}
                  >
                    <RefreshCwSpinIcon data-icon="inline-start" spinning={metaQuery.isFetching} />
                    {t("refresh")}
                  </Button>
                  <Button size="sm" onClick={() => void handleSave()} disabled={saving}>
                    {saving ? <Spinner /> : <SaveIcon data-icon="inline-start" />}
                    {t("save")}
                  </Button>
                </div>
              </div>

              <TabsContent
                value="general"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <section className="flex flex-col gap-5">
                    <FieldGroup>
                      <Field orientation="responsive">
                        <FieldContent>
                          <FieldTitle>{t("language")}</FieldTitle>
                          <FieldDescription>{t("chooseLanguage")}</FieldDescription>
                        </FieldContent>
                        <Select
                          value={draft.language}
                          onValueChange={(value) => patchDraft({ language: value as UiLanguage })}
                        >
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

                    <div className="flex flex-col gap-3">
                      <h4 className="text-sm font-medium">{t("coreBackend")}</h4>
                      <BackendFields draft={draft} patchDraft={patchDraft} />
                    </div>
                  </section>
                </ScrollPane>
              </TabsContent>

              <TabsContent
                value="connection"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <ConnectionSection draft={draft} patchDraft={patchDraft} />
                </ScrollPane>
              </TabsContent>

              <TabsContent
                value="diagnostics"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <section className="flex flex-col gap-5">
                    <FieldGroup>
                      <ToggleField
                        title={t("diagnosticsEnabled")}
                        description={t("diagnosticsEnabledDescription")}
                        checked={draft.diagnostics.enabled}
                        onCheckedChange={(enabled) => patchDiagnostics({ enabled })}
                      />
                      <ToggleField
                        title={t("runtimeLogEnabled")}
                        description={t("runtimeLogEnabledDescription")}
                        checked={draft.diagnostics.runtime_enabled}
                        disabled={!draft.diagnostics.enabled}
                        onCheckedChange={(enabled) =>
                          patchDiagnostics({ runtime_enabled: enabled })
                        }
                      />
                      <p className="text-xs text-muted-foreground">
                        {t("runtimeLogRestartNotice")}
                      </p>
                      <Field orientation="responsive">
                        <FieldContent>
                          <FieldTitle>{t("runtimeLogMaxSize")}</FieldTitle>
                          <FieldDescription>{t("runtimeLogMaxSizeDescription")}</FieldDescription>
                        </FieldContent>
                        <InputGroup className={compactInputGroupClass}>
                          <InputGroupInput
                            inputMode="decimal"
                            value={String(
                              draft.diagnostics.runtime_max_size_bytes / 1024 / 1024,
                            ).replace(/\.0$/, "")}
                            onChange={(event) =>
                              patchDiagnostics({
                                runtime_max_size_bytes: Math.max(
                                  1,
                                  Number(event.target.value || 1) * 1024 * 1024,
                                ),
                              })
                            }
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
                            onChange={(event) =>
                              patchDiagnostics({
                                runtime_retention_days:
                                  event.target.value === ""
                                    ? null
                                    : Math.max(0, Number(event.target.value)),
                              })
                            }
                          />
                          <InputGroupAddon align="inline-end">{t("days")}</InputGroupAddon>
                        </InputGroup>
                      </Field>
                    </FieldGroup>
                  </section>
                </ScrollPane>
              </TabsContent>

              <TabsContent
                value="cache"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <CacheSettingsSection />
                </ScrollPane>
              </TabsContent>

              <TabsContent
                value="config"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <section className="flex flex-col gap-3">
                    <p
                      className="truncate font-mono text-xs text-muted-foreground"
                      title={configFile?.path || undefined}
                    >
                      {configFile?.path || t("configFileMissing")}
                    </p>
                    <InputGroup>
                      <InputGroupTextarea
                        className="min-h-80 font-mono text-xs"
                        readOnly
                        value={configContent}
                      />
                    </InputGroup>
                  </section>
                </ScrollPane>
              </TabsContent>

              <TabsContent
                value="about"
                className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                  <section className="flex flex-col gap-5">
                    <McpstoreAsciiBanner />
                    <div className="flex items-start justify-between gap-4">
                      <p className="text-sm text-muted-foreground">{t("settingsDescription")}</p>
                      <MarkIcon className="size-8 shrink-0" />
                    </div>
                    <div className="divide-y">
                      <AboutRow label={t("appVersion")} value={formatReleaseVersion(appVersion)} />
                      <AboutRow
                        label={t("sdkVersion")}
                        value={formatReleaseVersion(meta?.version)}
                      />
                      {isDesktopShell ? (
                        <AboutRow
                          label={
                            updateAvailable
                              ? t("updateAvailable", { version: `v${updateAvailable}` })
                              : t("checkUpdates")
                          }
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
                      <AboutRow
                        label={t("github")}
                        href="https://github.com/ip2a/mcpstore"
                        value="https://github.com/ip2a/mcpstore"
                      />
                      <AboutRow
                        label={t("rustCrate")}
                        href="https://crates.io/crates/mcpstore"
                        value="https://crates.io/crates/mcpstore"
                      />
                      <AboutRow
                        label={t("pythonPackage")}
                        href="https://pypi.org/project/mcpstore/"
                        value="https://pypi.org/project/mcpstore/"
                      />
                      <AboutRow
                        label={t("npmPackage")}
                        href="https://www.npmjs.com/package/@ip2a/mcpstore"
                        value="https://www.npmjs.com/package/@ip2a/mcpstore"
                      />
                    </div>
                  </section>
                </ScrollPane>
              </TabsContent>
            </Tabs>
          ) : (
            <div className="p-4">
              <PageSkeleton />
            </div>
          )}
      </PanelCard>
    </>
  )
}

function CacheSettingsSection() {
  const { t } = useI18n()
  const setCacheDialogOpen = useUiStore((state) => state.setCacheDialogOpen)
  const healthQuery = useCacheHealthQuery()
  const backend =
    healthQuery.data &&
    typeof healthQuery.data === "object" &&
    healthQuery.data !== null &&
    "backend" in healthQuery.data &&
    typeof (healthQuery.data as { backend?: unknown }).backend === "string"
      ? (healthQuery.data as { backend: string }).backend
      : t("unknown")

  return (
    <section className="flex flex-col gap-5">
      <FieldGroup>
        <Field orientation="responsive">
          <FieldContent>
            <FieldTitle>{t("switchCacheStorage")}</FieldTitle>
            <FieldDescription>
              {t("currentCacheStorage", { backend })}
            </FieldDescription>
          </FieldContent>
          <Button size="sm" variant="outline" onClick={() => setCacheDialogOpen(true)}>
            <DatabaseIcon data-icon="inline-start" />
            {t("switchBackend")}
          </Button>
        </Field>
      </FieldGroup>
    </section>
  )
}

function ToggleField({
  title,
  description,
  checked,
  disabled,
  onCheckedChange,
}: {
  title: string
  description?: string
  checked: boolean
  disabled?: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <Field orientation="responsive">
      <FieldContent>
        <FieldTitle>{title}</FieldTitle>
        {description ? <FieldDescription>{description}</FieldDescription> : null}
      </FieldContent>
      <Switch
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
        aria-label={title}
      />
    </Field>
  )
}

function AboutRow({
  href,
  label,
  value,
  action,
}: {
  href?: string
  label: string
  value?: string
  action?: ReactNode
}) {
  return (
    <div className="flex min-w-0 items-center justify-between gap-4 py-3">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      {action ? (
        action
      ) : href ? (
        <a
          href={href}
          target="_blank"
          rel="noreferrer"
          className="min-w-0 truncate text-sm font-medium hover:underline"
        >
          {value}
        </a>
      ) : (
        <span className="min-w-0 truncate text-sm font-medium">{value}</span>
      )}
    </div>
  )
}

function BackendFields({
  draft,
  patchDraft,
}: {
  draft: SettingsDraft
  patchDraft: (patch: Partial<SettingsDraft>) => void
}) {
  const { t } = useI18n()
  return (
    <FieldGroup>
      <Field orientation="responsive">
        <FieldContent>
          <FieldTitle>{t("defaultBackendPort")}</FieldTitle>
          <FieldDescription>{t("defaultBackendPortDescription")}</FieldDescription>
        </FieldContent>
        <InputGroup className={compactInputGroupClass}>
          <InputGroupInput
            inputMode="numeric"
            value={draft.api.port}
            onChange={(event) =>
              patchDraft({
                api: { ...draft.api, port: Math.max(1, Number(event.target.value || 1)) },
              })
            }
          />
        </InputGroup>
      </Field>
      <Field orientation="responsive">
        <FieldContent>
          <FieldTitle>{t("defaultFrontendPort")}</FieldTitle>
          <FieldDescription>{t("defaultFrontendPortDescription")}</FieldDescription>
        </FieldContent>
        <InputGroup className={compactInputGroupClass}>
          <InputGroupInput
            inputMode="numeric"
            value={draft.web.port}
            onChange={(event) =>
              patchDraft({
                web: { ...draft.web, port: Math.max(1, Number(event.target.value || 1)) },
              })
            }
          />
        </InputGroup>
      </Field>
      <Field orientation="responsive">
        <FieldContent>
          <FieldTitle>{t("panelAuthSetting")}</FieldTitle>
          <FieldDescription>{t("panelAuthSettingDescription")}</FieldDescription>
        </FieldContent>
        <Switch
          checked={draft.web.auth}
          onCheckedChange={(checked) => patchDraft({ web: { ...draft.web, auth: checked } })}
        />
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
    const parsed =
      url.startsWith("http://") || url.startsWith("https://")
        ? new URL(url)
        : new URL(url, window.location.origin)
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

function parseConnectionUrl(
  url: string,
): { host: string; port: string; secure: boolean; path: string } | null {
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

function ConnectionSection({
  draft,
  patchDraft,
}: {
  draft: SettingsDraft
  patchDraft: (patch: Partial<SettingsDraft>) => void
}) {
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
  const [statuses, setStatuses] = useState<Record<string, HostStatus>>({})
  const [checking, setChecking] = useState<Record<string, boolean>>({})

  function patchHosts(
    nextEntries: Record<string, { urls: string[] }>,
    active = draft.hosts.active,
  ) {
    patchDraft({ hosts: { active, entries: nextEntries } })
  }

  async function refreshLatency(name: string, options?: { notify?: boolean }) {
    setChecking((current) => ({ ...current, [name]: true }))
    const status = await probeHost(name)
    setStatuses((current) => ({ ...current, [name]: status }))
    setChecking((current) => ({ ...current, [name]: false }))
    if (options?.notify) {
      if (status.online && status.latencyMs != null) {
        toast.success(t("connectionRefreshOk", { latency: formatLatency(status.latencyMs) }))
      } else {
        toast.error(t("connectionRefreshFailed"))
      }
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
    !Object.hasOwn(draft.hosts.entries, pendingName.trim()) &&
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
    if (name !== editingName && Object.hasOwn(draft.hosts.entries, name)) {
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

  const editingHost = editingName ? hosts.find((host) => host.name === editingName) : null

  return (
    <section className="flex flex-col gap-4">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4">
        {hosts.map((host) => {
          const isActive = host.name === draft.hosts.active
          const isEditing = editingName === host.name
          const canDelete = hosts.length > 1
          const isChecking = checking[host.name] === true
          const status = statuses[host.name]
          const online = status?.online === true
          const address = formatHostAddress(host.urls[0] ?? "")
          const detail =
            online && status?.store
              ? host.urls.length > 1
                ? `${status.store} · +${host.urls.length - 1}`
                : status.store
              : host.urls.length > 1
                ? `${host.urls[0]} +${host.urls.length - 1}`
                : (host.urls[0] ?? address)
          const metaParts = [
            status?.version ? `v${status.version}` : null,
            online && status?.latencyMs != null
              ? formatLatency(status.latencyMs)
              : isChecking
                ? "…"
                : null,
          ].filter(Boolean)

          return (
            <div
              key={host.name}
              role="button"
              tabIndex={0}
              className={cn(
                "group flex cursor-pointer flex-col rounded-lg border border-border/50 bg-card p-4 text-left",
                "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
                "transition-colors hover:border-border hover:bg-muted/40",
                isActive && "border-sky-500/35",
                isEditing && "border-border bg-muted/40",
              )}
              onClick={(event) => {
                if (event.target instanceof Element && event.target.closest("button")) return
                patchDraft({ hosts: { ...draft.hosts, active: host.name } })
              }}
              onKeyDown={(event) => {
                if (event.target instanceof Element && event.target.closest("button")) return
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault()
                  patchDraft({ hosts: { ...draft.hosts, active: host.name } })
                }
              }}
            >
              <div className="flex min-w-0 items-start gap-2">
                <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground">
                  <MonitorIcon className="size-4" aria-hidden />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="line-clamp-1 text-sm font-semibold leading-snug">
                    {host.name}
                  </span>
                  <span className="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                    <span
                      className={cn(
                        "size-1.5 shrink-0 rounded-full",
                        status == null
                          ? "bg-muted-foreground/40"
                          : online
                            ? "bg-emerald-500"
                            : "bg-destructive",
                      )}
                    />
                    <span className="truncate">
                      {status == null ? "…" : online ? t("online") : t("offline")}
                    </span>
                  </span>
                </span>
                <div className="flex shrink-0 items-center gap-0.5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t("refresh")}
                    disabled={isChecking}
                    onClick={() => void refreshLatency(host.name, { notify: true })}
                  >
                    <RefreshCwSpinIcon spinning={isChecking} className="size-3.5" />
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
                    <Trash2Icon
                      className={cn(
                        "size-3.5",
                        canDelete ? "text-destructive" : "text-muted-foreground",
                      )}
                    />
                  </Button>
                </div>
              </div>

              <div
                className="mt-3 min-h-10 min-w-0 w-full text-xs leading-5 text-muted-foreground"
                title={detail}
              >
                <p className="truncate">{address || t("noDescription")}</p>
                {detail && detail !== address ? <p className="truncate">{detail}</p> : null}
              </div>

              {metaParts.length > 0 ? (
                <p
                  className="mt-3 min-w-0 truncate font-mono text-xs tabular-nums text-muted-foreground"
                  title={metaParts.join(" · ")}
                >
                  {metaParts.join(" · ")}
                </p>
              ) : (
                <Skeleton className="mt-3 h-4 w-1/2" aria-hidden />
              )}
            </div>
          )
        })}

        <div
          role="button"
          tabIndex={0}
          className="group flex cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/70 bg-card/40 p-4 text-muted-foreground transition-colors hover:border-border hover:bg-muted/40 hover:text-foreground"
          onClick={() => {
            setEditingName(null)
            setEditName("")
            setEditUrls("")
            resetPending()
            setAddingOpen(true)
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault()
              setEditingName(null)
              setEditName("")
              setEditUrls("")
              resetPending()
              setAddingOpen(true)
            }
          }}
        >
          <span className="flex size-8 items-center justify-center rounded-md border border-dashed border-border/70">
            <PlusIcon className="size-4" />
          </span>
          <span className="text-sm font-medium">{t("addHost")}</span>
        </div>
      </div>

      {editingHost ? (
        <div className="rounded-lg border border-border/50 bg-card p-4">
          <p className="mb-3 text-sm font-medium">{t("edit")}</p>
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor={`host-name-${editingHost.name}`}>{t("host")}</Label>
              <InputGroup>
                <InputGroupInput
                  id={`host-name-${editingHost.name}`}
                  value={editName}
                  onChange={(event) => setEditName(event.target.value)}
                  placeholder={t("host")}
                />
              </InputGroup>
            </div>
            <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <Label htmlFor={`host-url-${editingHost.name}`}>{t("coreBackendUrlLabel")}</Label>
                <InputGroup>
                  <InputGroupTextarea
                    id={`host-url-${editingHost.name}`}
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
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => setEditingName(null)}
                >
                  {t("cancel")}
                </Button>
              </div>
            </div>
          </div>
        </div>
      ) : null}

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
                      Object.hasOwn(draft.hosts.entries, pendingName.trim()) &&
                      "border-destructive/50",
                  )}
                />
              </InputGroup>
            </div>

            {pendingFullUrlMode ? (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="connection-full-url">{t("fullUrl")}</Label>
                <div className="flex items-center gap-2">
                  <span className="shrink-0 font-mono text-sm text-muted-foreground">
                    {pendingSecure ? "https://" : "http://"}
                  </span>
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
                      className={cn(
                        pendingFullUrlBody.trim() && !parsedRawUrl && "border-destructive/50",
                      )}
                    />
                  </InputGroup>
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-4 sm:flex-row">
                <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                  <Label htmlFor="connection-host">{t("coreBackendUrlLabel")}</Label>
                  <div className="flex items-center gap-2">
                    <span className="shrink-0 font-mono text-sm text-muted-foreground">
                      {pendingSecure ? "https://" : "http://"}
                    </span>
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
              <div className="flex items-center gap-2 text-sm">
                <Switch
                  checked={pendingSecure}
                  onCheckedChange={setPendingSecure}
                  aria-label={t("useHttps")}
                />
                {t("useHttps")}
              </div>
              <div className="flex items-center gap-2 text-sm">
                <Switch
                  checked={pendingFullUrlMode}
                  onCheckedChange={setFullUrlMode}
                  aria-label={t("useFullUrl")}
                />
                {t("useFullUrl")}
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => handleAddDialogOpenChange(false)}
            >
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
