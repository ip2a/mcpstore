import { useEffect, useMemo, useState, type FormEvent } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { AlertCircleIcon, RefreshCwIcon, SaveIcon, SettingsIcon } from "lucide-react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { PathText } from "@/components/shared/path-text"
import { WorkspaceIdentity } from "@/components/shared/workspace-identity"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldContent, FieldDescription, FieldGroup, FieldTitle } from "@/components/ui/field"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { getMeta, updateSettings, type SettingsPayload, type UiLanguage, type UpdateSettingsPayload } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import type { I18nKey } from "@/lib/i18n-core"
import { queryKeys } from "@/lib/query-keys"

type SectionId = "general" | "config" | "about"

type SettingsDraft = {
  language: UiLanguage
  default_backup_dir: string
  logging: {
    max_size_bytes: number | null
    retention_days: number | null
  }
}

const sections: Array<{ id: SectionId; labelKey: I18nKey }> = [
  { id: "general", labelKey: "general" },
  { id: "config", labelKey: "configFile" },
  { id: "about", labelKey: "about" },
]

function settingsDraft(settings?: SettingsPayload): SettingsDraft {
  return {
    language: settings?.language || "auto",
    default_backup_dir: typeof settings?.default_backup_dir === "string" ? settings.default_backup_dir : "./backups",
    logging: {
      max_size_bytes: typeof settings?.logging?.max_size_bytes === "number" ? settings.logging.max_size_bytes : 5 * 1024 * 1024,
      retention_days: typeof settings?.logging?.retention_days === "number" ? settings.logging.retention_days : null,
    },
  }
}

function logSizeMb(draft: SettingsDraft) {
  const bytes = Number(draft.logging.max_size_bytes || 0)
  return String((bytes > 0 ? bytes : 5 * 1024 * 1024) / 1024 / 1024).replace(/\.0$/, "")
}

function payloadFromDraft(draft: SettingsDraft): UpdateSettingsPayload {
  return {
    language: draft.language,
    default_backup_dir: draft.default_backup_dir || "./backups",
    logging: {
      max_size_bytes: draft.logging.max_size_bytes,
      retention_days: draft.logging.retention_days,
    },
  }
}

export function SettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { setLanguageOverride, t } = useI18n()
  const queryClient = useQueryClient()
  const [section, setSection] = useState<SectionId>("general")
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const metaQuery = useQuery({
    enabled: open,
    queryKey: queryKeys.meta,
    queryFn: getMeta,
  })
  const settingsMutation = useMutation({ mutationKey: queryKeys.settings, mutationFn: updateSettings })
  const meta = metaQuery.data
  const loading = metaQuery.isFetching && !draft
  const error = metaQuery.error instanceof Error ? metaQuery.error.message : ""
  const saving = settingsMutation.isPending

  const settingsPaths = meta?.settings_paths
  const configFile = meta?.config_file
  const configContent = useMemo(() => configFile?.content || "", [configFile?.content])

  useEffect(() => {
    if (open && meta) setDraft(settingsDraft(meta.settings))
  }, [meta, open])

  function patchDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => (current ? { ...current, ...patch } : current))
  }

  function patchLogging(patch: Partial<SettingsDraft["logging"]>) {
    setDraft((current) => (current ? { ...current, logging: { ...current.logging, ...patch } } : current))
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!draft) return
    try {
      await settingsMutation.mutateAsync(payloadFromDraft(draft))
      setLanguageOverride(draft.language)
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
      <DialogContent className="flex h-[min(720px,calc(100dvh-32px))] flex-col gap-0 p-0 sm:max-w-3xl">
        <DialogHeader className="border-b px-4 py-3 sm:px-5">
          <DialogTitle className="flex items-center gap-2">
            <SettingsIcon className="size-4" />
            {t("settings")}
          </DialogTitle>
          <DialogDescription>{t("settingsDescription")}</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-[144px_minmax(0,1fr)]">
          <nav className="flex flex-col gap-1 border-r p-3" aria-label={t("settingsNav")}>
            {sections.map((item) => (
              <Button
                key={item.id}
                type="button"
                variant={section === item.id ? "secondary" : "ghost"}
                className="justify-start"
                onClick={() => setSection(item.id)}
              >
                {t(item.labelKey)}
              </Button>
            ))}
          </nav>

          <ScrollArea className="min-h-0">
            <div className="p-4 sm:p-5">
              {loading ? <SettingsLoading label={t("loadingSettings")} /> : null}
              {error ? <SettingsError message={error} onRetry={() => void metaQuery.refetch()} /> : null}

              {!loading && !error && draft ? (
                <DialogForm onSubmit={onSubmit}>
                  {section === "general" ? (
                    <section className="flex flex-col gap-5">
                      <SectionHead title={t("general")} description={t("generalDescription")} />
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
                                <SelectItem value="zh">中文</SelectItem>
                                <SelectItem value="en">English</SelectItem>
                              </SelectGroup>
                            </SelectContent>
                          </Select>
                        </Field>

                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("defaultBackupDir")}</FieldTitle>
                            <FieldDescription>
                              <PathText value={settingsPaths?.backup_dir_resolved} fallback={t("backupDirMissing")} wrap="all" />
                            </FieldDescription>
                          </FieldContent>
                          <InputGroup className="max-w-xl">
                            {settingsPaths?.backup_dir_base ? (
                              <InputGroupAddon align="inline-start" className="max-w-48 truncate">
                                <PathText value={settingsPaths.backup_dir_base} wrap="truncate" />
                              </InputGroupAddon>
                            ) : null}
                            <InputGroupInput value={draft.default_backup_dir} onChange={(event) => patchDraft({ default_backup_dir: event.target.value })} placeholder="./backups" />
                          </InputGroup>
                        </Field>

                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("logMaxSizeMb")}</FieldTitle>
                            <FieldDescription>
                              <PathText value={settingsPaths?.log_file_path} fallback={t("logFilePathMissing")} wrap="all" />
                            </FieldDescription>
                          </FieldContent>
                          <InputGroup className="w-32">
                            <InputGroupInput
                              inputMode="decimal"
                              value={logSizeMb(draft)}
                              onChange={(event) => patchLogging({ max_size_bytes: Math.max(0, Number(event.target.value || 0) * 1024 * 1024) })}
                            />
                            <InputGroupAddon align="inline-end">MB</InputGroupAddon>
                          </InputGroup>
                        </Field>

                        <Field orientation="responsive">
                          <FieldContent>
                            <FieldTitle>{t("logRetentionDays")}</FieldTitle>
                            <FieldDescription>{t("unlimited")}</FieldDescription>
                          </FieldContent>
                          <InputGroup className="w-32">
                            <InputGroupInput
                              inputMode="numeric"
                              placeholder="不限"
                              value={draft.logging.retention_days ?? ""}
                              onChange={(event) => patchLogging({ retention_days: event.target.value === "" ? null : Math.max(0, Number(event.target.value)) })}
                            />
                            <InputGroupAddon align="inline-end">days</InputGroupAddon>
                          </InputGroup>
                        </Field>
                      </FieldGroup>
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

                  {section === "about" ? (
                    <section className="flex flex-col gap-4">
                      <SectionHead title={t("about")} description={t("settingsDescription")} />
                      <ReadonlyValue label={t("version")} value={meta?.version ? `v${meta.version}` : "-"} />
                      <ReadonlyValue label={t("metaApi")} value="/api/v1/meta" />
                      <ReadonlyValue label={t("settingsApi")} value="PUT /api/v1/settings" />
                    </section>
                  ) : null}

                  <DialogFormFooter
                    className="mt-auto border-t pt-4"
                    onCancel={() => handleOpenChange(false)}
                    submitDisabled={!draft}
                    submitLabel={
                      <>
                        {!saving ? <SaveIcon data-icon="inline-start" /> : null}
                        {t("save")}
                      </>
                    }
                    submitting={saving}
                  />
                </DialogForm>
              ) : null}
            </div>
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function SectionHead({ title, description }: { title: string; description: string }) {
  return (
    <div className="border-b pb-3">
      <h3 className="text-base font-semibold">{title}</h3>
      <p className="mt-1 text-sm text-muted-foreground">{description}</p>
    </div>
  )
}

function ReadonlyValue({ label, value, path = false }: { label: string; value: string; path?: boolean }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-md border p-3">
      <span className="text-sm text-muted-foreground">{label}</span>
      {path ? <PathText value={value} tone="default" weight="medium" wrap="all" /> : <code className="truncate text-sm">{value}</code>}
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
