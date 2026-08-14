import { useEffect, useMemo, useState, type FormEvent } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ActivityIcon, AlertCircleIcon, ChevronRightIcon, FileTextIcon, InfoIcon, NetworkIcon, PencilIcon, PlusIcon, RefreshCwIcon, SaveIcon, SettingsIcon, SlidersHorizontalIcon, Trash2Icon, type LucideIcon } from "lucide-react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { WorkspaceIdentity } from "@/components/shared/workspace-identity"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldContent, FieldDescription, FieldGroup, FieldTitle } from "@/components/ui/field"
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
        <DialogHeader className="border-b px-4 py-3 sm:px-5">
          <DialogTitle className="flex items-center gap-2">
            <SettingsIcon className="size-4" />
            {t("settings")}
          </DialogTitle>
          <DialogDescription>{t("settingsDescription")}</DialogDescription>
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

function SectionHead({ title, description }: { title: string; description?: string }) {
  return (
    <div className="border-b pb-3">
      <h3 className="text-base font-semibold">{title}</h3>
      {description ? <p className="mt-1 text-sm text-muted-foreground">{description}</p> : null}
    </div>
  )
}

function ToggleField({ title, description, checked, disabled, onCheckedChange }: { title: string; description: string; checked: boolean; disabled?: boolean; onCheckedChange: (checked: boolean) => void }) {
  return (
    <Field orientation="responsive">
      <FieldContent><FieldTitle>{title}</FieldTitle><FieldDescription>{description}</FieldDescription></FieldContent>
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

function ConnectionSection({ draft, patchDraft }: { draft: SettingsDraft; patchDraft: (patch: Partial<SettingsDraft>) => void }) {
  const { t } = useI18n()
  const [addingOpen, setAddingOpen] = useState(false)
  const [pendingUrl, setPendingUrl] = useState("")
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

  function addConnection() {
    const url = pendingUrl.trim()
    if (!url) return
    const connection: ConnectionDraft = { id: crypto.randomUUID(), url }
    patchDraft({ connections: [...draft.connections, connection] })
    setPendingUrl("")
    setAddingOpen(false)
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
      <SectionHead title={t("connection")} description={t("connectionDescription")} />

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

      <Collapsible open={addingOpen} onOpenChange={setAddingOpen}>
        {!addingOpen ? (
          <Button type="button" variant="outline" className="w-fit" onClick={() => setAddingOpen(true)}>
            <PlusIcon data-icon="inline-start" />
            {t("connectionAdd")}
          </Button>
        ) : null}
        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-out data-[state=open]:animate-in">
          <div className="rounded-md border">
            <CollapsibleTrigger className="flex w-full items-center gap-2 px-3 py-2.5 text-left text-sm font-medium transition-colors hover:bg-muted/50">
              <ChevronRightIcon className={cn("size-4 shrink-0 text-muted-foreground transition-transform", addingOpen && "rotate-90")} />
              {t("connectionPending")}
            </CollapsibleTrigger>
            <div className="flex flex-col gap-3 border-t px-3 py-3 sm:flex-row sm:items-center">
              <InputGroup className="min-w-0 flex-1">
                <InputGroupInput
                  value={pendingUrl}
                  onChange={(event) => setPendingUrl(event.target.value)}
                  placeholder={t("coreBackendUrlPlaceholder")}
                />
              </InputGroup>
              <div className="flex gap-2">
                <Button type="button" size="sm" onClick={addConnection} disabled={!pendingUrl.trim()}>
                  {t("add")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    setPendingUrl("")
                    setAddingOpen(false)
                  }}
                >
                  {t("cancel")}
                </Button>
              </div>
            </div>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </section>
  )
}
