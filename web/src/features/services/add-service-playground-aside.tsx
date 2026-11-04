import { useEffect, useMemo, useState } from "react"

import { buildAddServiceCliCommand } from "@/features/services/build-add-service-cli"
import { ServiceRestartPolicySelect, ServiceStartupPolicySelect } from "@/features/services/service-lifecycle-fields"
import {
  configToFields,
  HANDSHAKE_MODES,
  parseImportedServiceConfig,
  serializeConfigFields,
  type HandshakeMode,
  type ServiceConfigFields,
  type ServiceConfigFormat,
} from "@/features/services/service-config-draft"
import type { AddServiceScope } from "@/features/services/use-add-service-form"
import { AgentIdPicker } from "@/components/shared/agent-id-picker"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockBody,
  CodeBlockCopyButton,
  CodeBlockFooter,
  CodeBlockHeader,
  CodeBlockMethod,
  CodeBlockString,
  CodeBlockTitle,
} from "@/components/ui/code-block"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { TypographyLead } from "@/components/ui/typography"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"
import { cn } from "@/lib/utils"

type StoreAsideTab = "preview" | "description" | "handshake" | "scope" | "lifecycle"

export function ServiceConfigPreviewPane({
  agentId,
  className,
  fields,
  name,
  onFieldsChange,
  onNameChange,
  previewFormat,
  onPreviewFormatChange,
  scope,
  showCli = true,
}: {
  agentId: string
  className?: string
  fields: ServiceConfigFields
  name: string
  onFieldsChange: (fields: ServiceConfigFields) => void
  onNameChange?: (name: string) => void
  previewFormat: ServiceConfigFormat
  onPreviewFormatChange: (format: ServiceConfigFormat) => void
  scope: AddServiceScope
  showCli?: boolean
}) {
  const { t } = useI18n()
  const cliCommand = useMemo(
    () => buildAddServiceCliCommand({ name, scope, agentId, fields }),
    [agentId, fields, name, scope],
  )
  const serialized = useMemo(() => serializeConfigFields(fields, previewFormat), [fields, previewFormat])
  const [previewText, setPreviewText] = useState(serialized)
  const [parseError, setParseError] = useState<string | null>(null)
  const [previewDirty, setPreviewDirty] = useState(false)

  useEffect(() => {
    if (!previewDirty) {
      setPreviewText(serialized)
      setParseError(null)
    }
  }, [previewDirty, serialized])

  useEffect(() => {
    setPreviewDirty(false)
  }, [previewFormat])

  function onPreviewChange(text: string) {
    setPreviewText(text)
    setPreviewDirty(true)
    try {
      const imported = parseImportedServiceConfig(previewFormat, text)
      onFieldsChange(configToFields(imported.config))
      if (imported.name?.trim()) {
        onNameChange?.(imported.name.trim())
      }
      setParseError(null)
      setPreviewDirty(false)
    } catch (err) {
      setParseError(err instanceof Error ? err.message : "Invalid config")
    }
  }

  const serviceName = name.trim() || "github"
  const previewHint =
    parseError || (previewFormat === "json" ? t("fieldHelpJson") : t("fieldHelpToml"))

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col gap-3 overflow-hidden @min-[640px]:gap-4", className)}>
      {showCli ? (
        <CodeBlock variant="request" className="flex shrink-0 flex-col">
          <CodeBlockHeader variant="request">
            <CodeBlockTitle>
              <CodeBlockMethod className="bg-sky-500/15 text-sky-300">ADD</CodeBlockMethod>
              <span className="truncate text-zinc-300">{serviceName}</span>
            </CodeBlockTitle>
            <CodeBlockActions>
              <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">
                {t("cliRequestLabel")}
              </span>
              <CodeBlockCopyButton value={cliCommand} className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50" />
            </CodeBlockActions>
          </CodeBlockHeader>
          <CodeBlockBody variant="request" maxHeight="9rem">
            <CodeBlockString lines={cliCommand.split("\n")} />
          </CodeBlockBody>
        </CodeBlock>
      ) : null}

      <CodeBlock
        variant="response"
        className="flex min-h-0 flex-1 flex-col overflow-hidden"
      >
        <CodeBlockHeader variant="response" className="shrink-0">
          <Tabs
            value={previewFormat}
            onValueChange={(value) => onPreviewFormatChange(value as ServiceConfigFormat)}
            className="gap-0"
          >
            <TabsList className="h-8 w-fit">
              <TabsTrigger value="json">json</TabsTrigger>
              <TabsTrigger value="toml">toml</TabsTrigger>
            </TabsList>
          </Tabs>
          <CodeBlockActions>
            <CodeBlockCopyButton value={previewText} />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="response" fill bare className="p-0">
          <textarea
            value={previewText}
            spellCheck={false}
            className="block h-full min-h-0 w-full flex-1 resize-none overflow-auto border-0 bg-transparent p-4 font-mono text-sm leading-relaxed text-foreground outline-none"
            onChange={(event) => onPreviewChange(event.target.value)}
          />
        </CodeBlockBody>
        <CodeBlockFooter variant="response" className="shrink-0">
          <TypographyLead className={cn("m-0 text-xs", parseError ? "text-destructive" : "text-muted-foreground")}>
            {previewHint}
          </TypographyLead>
        </CodeBlockFooter>
      </CodeBlock>
    </div>
  )
}

export function ServiceConfigDescriptionPane({
  fields,
  onFieldsChange,
}: {
  fields: ServiceConfigFields
  onFieldsChange: (fields: ServiceConfigFields) => void
}) {
  const { t } = useI18n()

  return (
    <Field>
      <FieldLabel htmlFor="aside-config-description">{t("description")}</FieldLabel>
      <Input
        id="aside-config-description"
        value={fields.description}
        placeholder={t("optionalDescription")}
        onChange={(event) => onFieldsChange({ ...fields, description: event.target.value })}
      />
    </Field>
  )
}

export function ServiceConfigHandshakePane({
  fields,
  onFieldsChange,
}: {
  fields: ServiceConfigFields
  onFieldsChange: (fields: ServiceConfigFields) => void
}) {
  const { t } = useI18n()

  return (
    <FieldGroup>
      <Field orientation="responsive">
        <FieldLabel htmlFor="aside-config-handshake-mode">{t("handshakeMode")}</FieldLabel>
        <Select
          value={fields.handshakeMode}
          onValueChange={(value) =>
            onFieldsChange({ ...fields, handshakeMode: value as HandshakeMode })
          }
        >
          <SelectTrigger id="aside-config-handshake-mode" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {HANDSHAKE_MODES.map((mode) => (
                <SelectItem key={mode} value={mode}>
                  {t(`handshake${mode.charAt(0).toUpperCase()}${mode.slice(1)}`)}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      </Field>
      <span className="text-xs text-muted-foreground">{t("handshakeModeHint")}</span>
    </FieldGroup>
  )
}

export function AddServicePlaygroundAside({
  agentId,
  agentIds = [],
  className,
  fields,
  name,
  onAgentIdChange,
  onFieldsChange,
  onNameChange,
  onRestartPolicyChange,
  onScopeChange,
  onStartupPolicyChange,
  previewFormat,
  onPreviewFormatChange,
  restartPolicy,
  scope,
  showCli = true,
  showLifecycleTab = false,
  showScopeTab = false,
  startupPolicy,
}: {
  agentId: string
  agentIds?: string[]
  className?: string
  fields: ServiceConfigFields
  name: string
  onAgentIdChange?: (agentId: string) => void
  onFieldsChange: (fields: ServiceConfigFields) => void
  onNameChange?: (name: string) => void
  onRestartPolicyChange?: (policy: ServiceRestartPolicy) => void
  onScopeChange?: (scope: AddServiceScope) => void
  onStartupPolicyChange?: (policy: ServiceStartupPolicy) => void
  previewFormat: ServiceConfigFormat
  onPreviewFormatChange: (format: ServiceConfigFormat) => void
  restartPolicy?: ServiceRestartPolicy
  scope: AddServiceScope
  showCli?: boolean
  showLifecycleTab?: boolean
  showScopeTab?: boolean
  startupPolicy?: ServiceStartupPolicy
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<StoreAsideTab>("preview")

  return (
    <aside className={cn("flex h-full min-h-0 min-w-0 w-full flex-col", className)}>
      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as StoreAsideTab)}
        className="flex min-h-0 flex-1 flex-col gap-3 @min-[640px]:gap-4"
      >
        <TabsList variant="line" className="h-9 w-full shrink-0 justify-start overflow-x-auto">
          <TabsTrigger value="preview">Preview</TabsTrigger>
          <TabsTrigger value="description">{t("description")}</TabsTrigger>
          <TabsTrigger value="handshake">{t("handshakeMode")}</TabsTrigger>
          {showScopeTab ? <TabsTrigger value="scope">{t("scope")}</TabsTrigger> : null}
          {showLifecycleTab ? <TabsTrigger value="lifecycle">{t("lifecyclePolicy")}</TabsTrigger> : null}
        </TabsList>

        <TabsContent value="preview" className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <ServiceConfigPreviewPane
            agentId={agentId}
            fields={fields}
            name={name}
            previewFormat={previewFormat}
            scope={scope}
            showCli={showCli}
            onFieldsChange={onFieldsChange}
            onNameChange={onNameChange}
            onPreviewFormatChange={onPreviewFormatChange}
          />
        </TabsContent>

        <TabsContent value="description" className="min-h-0 flex-1 overflow-hidden">
          <ServiceConfigDescriptionPane fields={fields} onFieldsChange={onFieldsChange} />
        </TabsContent>

        <TabsContent value="handshake" className="min-h-0 flex-1 overflow-hidden">
          <ServiceConfigHandshakePane fields={fields} onFieldsChange={onFieldsChange} />
        </TabsContent>

        {showScopeTab ? (
          <TabsContent value="scope" className="min-h-0 flex-1 overflow-hidden">
            <FieldGroup>
              <div className="flex items-center gap-3">
                <span className="shrink-0 text-sm text-muted-foreground">{t("agent")}</span>
                <Switch
                  id="aside-add-service-agent-scope"
                  checked={scope === "agent"}
                  onCheckedChange={(useAgent) => {
                    onScopeChange?.(useAgent ? "agent" : "store")
                    if (!useAgent) {
                      onAgentIdChange?.("")
                    }
                  }}
                  aria-label={t("agent")}
                />
                <AgentIdPicker
                  id="aside-agentId"
                  name="agentId"
                  agentIds={agentIds}
                  value={agentId}
                  disabled={scope !== "agent"}
                  required={scope === "agent"}
                  placeholder={scope === "agent" ? t("agentIdPlaceholder") : t("store")}
                  onChange={(next) => onAgentIdChange?.(next)}
                />
              </div>
            </FieldGroup>
          </TabsContent>
        ) : null}

        {showLifecycleTab && startupPolicy && restartPolicy ? (
          <TabsContent value="lifecycle" className="min-h-0 flex-1 overflow-hidden">
            <FieldGroup className="gap-4">
              <ServiceStartupPolicySelect value={startupPolicy} onChange={(next) => onStartupPolicyChange?.(next)} />
              <ServiceRestartPolicySelect value={restartPolicy} onChange={(next) => onRestartPolicyChange?.(next)} />
            </FieldGroup>
          </TabsContent>
        ) : null}
      </Tabs>
    </aside>
  )
}
