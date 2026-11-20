import { useMemo, useState } from "react"

import { buildAddServiceCliCommand } from "@/features/services/build-add-service-cli"
import {
  ServiceRestartPolicySelect,
  ServiceStartupPolicySelect,
} from "@/features/services/service-lifecycle-fields"
import {
  HANDSHAKE_MODES,
  type HandshakeMode,
  type ServiceConfigFields,
  type ServiceConfigFormat,
} from "@/features/services/service-config-draft"
import { ServiceConfigPreviewCard } from "@/features/services/service-config-preview-card"
import type { AddServiceScope } from "@/features/services/use-add-service-form"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Textarea } from "@/components/ui/textarea"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"
import { cn } from "@/lib/utils"

type StoreAsideTab = "preview" | "profile"

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
  const cliCommand = useMemo(
    () => buildAddServiceCliCommand({ name, scope, agentId, fields }),
    [agentId, fields, name, scope],
  )

  return (
    <div
      className={cn(
        "flex min-h-0 flex-1 flex-col gap-3 overflow-hidden @min-[640px]:gap-4",
        className,
      )}
    >
      <ServiceConfigPreviewCard
        cliCommand={cliCommand}
        fields={fields}
        previewFormat={previewFormat}
        showCli={showCli}
        onFieldsChange={onFieldsChange}
        onNameChange={onNameChange}
        onPreviewFormatChange={onPreviewFormatChange}
      />
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
      <Textarea
        id="aside-config-description"
        rows={2}
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
    <FieldGroup className="gap-2">
      <Field
        orientation="horizontal"
        className="justify-between [&>[data-slot=field-label]]:flex-none"
      >
        <FieldLabel htmlFor="aside-config-handshake-mode">{t("handshakeMode")}</FieldLabel>
        <Select
          value={fields.handshakeMode}
          onValueChange={(value) =>
            onFieldsChange({ ...fields, handshakeMode: value as HandshakeMode })
          }
        >
          <SelectTrigger id="aside-config-handshake-mode" className="w-52 max-w-[13rem]">
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
      <span className="text-xs text-muted-foreground">
        {t(
          `handshake${fields.handshakeMode.charAt(0).toUpperCase()}${fields.handshakeMode.slice(1)}Hint`,
        )}
      </span>
    </FieldGroup>
  )
}

export function AddServicePlaygroundAside({
  agentId,
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
  scopeOptions = [],
  showCli = true,
  showLifecycleTab = false,
  showScopeTab = false,
  startupPolicy,
}: {
  agentId: string
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
  /** Writable scopes only: store + agent ids (same source as Services tabs). */
  scopeOptions?: Array<{ value: string; label: string }>
  showCli?: boolean
  showLifecycleTab?: boolean
  showScopeTab?: boolean
  startupPolicy?: ServiceStartupPolicy
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<StoreAsideTab>("preview")
  const scopeValue = scope === "agent" && agentId.trim() ? agentId.trim() : "store"
  const resolvedScopeOptions = useMemo(() => {
    const options = [...scopeOptions]
    if (!options.some((option) => option.value === "store")) {
      options.unshift({ value: "store", label: t("store") })
    }
    if (scopeValue !== "store" && !options.some((option) => option.value === scopeValue)) {
      options.push({ value: scopeValue, label: scopeValue })
    }
    return options
  }, [scopeOptions, scopeValue, t])

  return (
    <aside className={cn("flex h-full min-h-0 min-w-0 w-full flex-col", className)}>
      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as StoreAsideTab)}
        className="flex min-h-0 flex-1 flex-col gap-3 @min-[640px]:gap-4"
      >
        <TabsList
          variant="line"
          className="h-9 w-fit max-w-full shrink-0 justify-start overflow-x-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
        >
          <TabsTrigger value="preview" className="flex-none">
            Preview
          </TabsTrigger>
          <TabsTrigger value="profile" className="flex-none">
            {t("profile")}
          </TabsTrigger>
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

        <TabsContent value="profile" className="min-h-0 flex-1 overflow-hidden">
          <div className="flex h-full min-h-0 flex-col gap-5 overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
            {showScopeTab ? (
              <FieldGroup className="gap-2">
                <Field
                  orientation="horizontal"
                  className="justify-between [&>[data-slot=field-label]]:flex-none"
                >
                  <FieldLabel htmlFor="aside-add-service-scope">{t("scope")}</FieldLabel>
                  <Select
                    value={scopeValue}
                    onValueChange={(value) => {
                      if (value === "store") {
                        onScopeChange?.("store")
                        onAgentIdChange?.("")
                        return
                      }
                      onScopeChange?.("agent")
                      onAgentIdChange?.(value)
                    }}
                  >
                    <SelectTrigger id="aside-add-service-scope" className="w-40 max-w-[10rem]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {resolvedScopeOptions.map((option) => (
                          <SelectItem key={option.value} value={option.value}>
                            {option.label}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <span className="text-xs text-muted-foreground">
                  {scopeValue === "store"
                    ? t("scopeStoreHint")
                    : t("scopeAgentHint", { scope: scopeValue })}
                </span>
              </FieldGroup>
            ) : null}
            <ServiceConfigHandshakePane fields={fields} onFieldsChange={onFieldsChange} />
            {showLifecycleTab && startupPolicy && restartPolicy ? (
              <FieldGroup className="gap-4">
                <ServiceStartupPolicySelect
                  value={startupPolicy}
                  onChange={(next) => onStartupPolicyChange?.(next)}
                />
                <ServiceRestartPolicySelect
                  value={restartPolicy}
                  onChange={(next) => onRestartPolicyChange?.(next)}
                />
              </FieldGroup>
            ) : null}
            <ServiceConfigDescriptionPane fields={fields} onFieldsChange={onFieldsChange} />
          </div>
        </TabsContent>
      </Tabs>
    </aside>
  )
}
