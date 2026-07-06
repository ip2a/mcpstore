import { useEffect, useMemo, useState, type FormEvent } from "react"
import { AlertCircleIcon, RefreshCwIcon, SaveIcon, SettingsIcon } from "lucide-react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { PathText } from "@/components/shared/path-text"
import { WorkspaceIdentity } from "@/components/shared/workspace-identity"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldContent, FieldDescription, FieldGroup, FieldTitle } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { getMeta, updateSettings, type MetaPayload, type SettingsPayload, type UiLanguage, type UpdateSettingsPayload } from "@/lib/api"

type SectionId = "general" | "config" | "about"

type SettingsDraft = {
  language: UiLanguage
  default_backup_dir: string
  logging: {
    max_size_bytes: number | null
    retention_days: number | null
  }
}

const sections: Array<{ id: SectionId; label: string }> = [
  { id: "general", label: "通用" },
  { id: "config", label: "配置文件" },
  { id: "about", label: "关于" },
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
  const [section, setSection] = useState<SectionId>("general")
  const [meta, setMeta] = useState<MetaPayload | null>(null)
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState("")

  const settingsPaths = meta?.settings_paths
  const configFile = meta?.config_file
  const configContent = useMemo(() => configFile?.content || "", [configFile?.content])

  async function loadSettings() {
    setLoading(true)
    setError("")
    try {
      const nextMeta = await getMeta()
      setMeta(nextMeta)
      setDraft(settingsDraft(nextMeta.settings))
    } catch (err) {
      setError(err instanceof Error ? err.message : "设置服务暂不可用")
      setMeta(null)
      setDraft(null)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (open) void loadSettings()
  }, [open])

  function patchDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => (current ? { ...current, ...patch } : current))
  }

  function patchLogging(patch: Partial<SettingsDraft["logging"]>) {
    setDraft((current) => (current ? { ...current, logging: { ...current.logging, ...patch } } : current))
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!draft) return
    setSaving(true)
    try {
      await updateSettings(payloadFromDraft(draft))
      toast.success("设置已保存")
      await loadSettings()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "设置保存失败")
    } finally {
      setSaving(false)
    }
  }

  function handleOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      setSection("general")
      setError("")
    }
    onOpenChange(nextOpen)
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex h-[min(720px,calc(100dvh-32px))] flex-col gap-0 p-0 sm:max-w-3xl">
        <DialogHeader className="border-b px-4 py-3 sm:px-5">
          <DialogTitle className="flex items-center gap-2">
            <SettingsIcon className="size-4" />
            设置
          </DialogTitle>
          <DialogDescription>管理 mcpstore Web 设置和配置文件信息。</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-[144px_minmax(0,1fr)]">
          <nav className="flex flex-col gap-1 border-r p-3" aria-label="设置">
            {sections.map((item) => (
              <Button
                key={item.id}
                type="button"
                variant={section === item.id ? "secondary" : "ghost"}
                className="justify-start"
                onClick={() => setSection(item.id)}
              >
                {item.label}
              </Button>
            ))}
          </nav>

          <div className="min-h-0 overflow-auto p-4 sm:p-5">
            {loading ? <SettingsLoading label="正在加载设置" /> : null}
            {error ? <SettingsError message={error} onRetry={loadSettings} /> : null}

            {!loading && !error && draft ? (
              <DialogForm onSubmit={onSubmit}>
                {section === "general" ? (
                  <section className="flex flex-col gap-5">
                    <SectionHead title="通用" description="这些字段会通过 /api/v1/settings 保存到后端。" />
                    <FieldGroup>
                      <Field orientation="responsive">
                        <FieldContent>
                          <FieldTitle>语言</FieldTitle>
                          <FieldDescription>控制设置界面的语言偏好；后端可按需使用该值。</FieldDescription>
                        </FieldContent>
                        <Select value={draft.language} onValueChange={(value) => patchDraft({ language: value as UiLanguage })}>
                          <SelectTrigger className="w-44">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectGroup>
                              <SelectItem value="auto">自动</SelectItem>
                              <SelectItem value="zh">中文</SelectItem>
                              <SelectItem value="en">English</SelectItem>
                            </SelectGroup>
                          </SelectContent>
                        </Select>
                      </Field>

                      <Field orientation="responsive">
                        <FieldContent>
                          <FieldTitle>默认备份目录</FieldTitle>
                          <FieldDescription>
                            <PathText value={settingsPaths?.backup_dir_resolved} fallback="后端未返回解析后的目录。" wrap="all" />
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
                          <FieldTitle>日志大小上限 MB</FieldTitle>
                          <FieldDescription>
                            <PathText value={settingsPaths?.log_file_path} fallback="日志路径会在后端 meta 接口完成后显示。" wrap="all" />
                          </FieldDescription>
                        </FieldContent>
                        <Input
                          className="w-32"
                          inputMode="decimal"
                          value={logSizeMb(draft)}
                          onChange={(event) => patchLogging({ max_size_bytes: Math.max(0, Number(event.target.value || 0) * 1024 * 1024) })}
                        />
                      </Field>

                      <Field orientation="responsive">
                        <FieldContent>
                          <FieldTitle>日志保留天数</FieldTitle>
                          <FieldDescription>留空表示不限制。</FieldDescription>
                        </FieldContent>
                        <Input
                          className="w-32"
                          inputMode="numeric"
                          placeholder="不限"
                          value={draft.logging.retention_days ?? ""}
                          onChange={(event) => patchLogging({ retention_days: event.target.value === "" ? null : Math.max(0, Number(event.target.value)) })}
                        />
                      </Field>
                    </FieldGroup>
                  </section>
                ) : null}

                {section === "config" ? (
                  <section className="flex flex-col gap-4">
                    <SectionHead title="配置文件" description="只读展示后端 meta 接口返回的配置文件内容。" />
                    <WorkspaceIdentity
                      workspace={configFile?.path}
                      fallbackTitle="未返回配置文件"
                      label="Config File"
                      className="rounded-md border p-3"
                    />
                    <Textarea className="min-h-80 font-mono text-xs" readOnly value={configContent} />
                  </section>
                ) : null}

                {section === "about" ? (
                  <section className="flex flex-col gap-4">
                    <SectionHead title="关于" description="当前 Web 设置接口状态。" />
                    <ReadonlyValue label="版本" value={meta?.version ? `v${meta.version}` : "-"} />
                    <ReadonlyValue label="Meta API" value="/api/v1/meta" />
                    <ReadonlyValue label="Settings API" value="PUT /api/v1/settings" />
                  </section>
                ) : null}

                <DialogFormFooter
                  className="mt-auto border-t pt-4"
                  onCancel={() => handleOpenChange(false)}
                  submitDisabled={!draft}
                  submitLabel={
                    <>
                      {!saving ? <SaveIcon data-icon="inline-start" /> : null}
                      保存
                    </>
                  }
                  submitting={saving}
                />
              </DialogForm>
            ) : null}
          </div>
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
  return (
    <div className="flex flex-col gap-3 rounded-md border border-destructive/30 p-4 text-sm">
      <div className="flex items-start gap-2 text-destructive">
        <AlertCircleIcon className="mt-0.5 size-4" />
        <div>
          <p className="font-medium">设置服务暂不可用</p>
          <p className="mt-1 text-muted-foreground">{message}</p>
        </div>
      </div>
      <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
        <RefreshCwIcon data-icon="inline-start" />
        重试
      </Button>
    </div>
  )
}
