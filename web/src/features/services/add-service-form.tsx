import { useAddServiceForm } from "@/features/services/use-add-service-form"
import { AddServicePlaygroundAside } from "@/features/services/add-service-playground-aside"
import { ServiceConfigFormFields } from "@/features/services/service-config-form-fields"
import { getUiTransportMode, resolveHttpTransport } from "@/features/services/service-config-draft"
import { ServiceRestartPolicySelect, ServiceStartupPolicySelect } from "@/features/services/service-lifecycle-fields"
import { DialogFormFooter } from "@/components/shared/dialog-form"
import { FieldCollapsible } from "@/components/shared/field-collapsible"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import { getAgentId } from "@/features/agents/model"
import { type AgentItem } from "@/lib/api"
import { cn } from "@/lib/utils"

export function AddServiceForm({
  agents,
  className,
  onAdded,
  onCancel,
  showActions = true,
}: {
  agents: AgentItem[]
  className?: string
  onAdded: () => Promise<void>
  onCancel?: () => void
  showActions?: boolean
}) {
  const { t } = useI18n()
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const {
    agentId,
    configFields,
    onSubmit,
    previewFormat,
    restartPolicy,
    scope,
    serviceName,
    setAgentId,
    setConfigFields,
    setPreviewFormat,
    setRestartPolicy,
    setScope,
    setServiceName,
    setStartupPolicy,
    startupPolicy,
    submitting,
  } = useAddServiceForm({
    onAdded,
    onBack: () => onCancel?.(),
  })

  const connectionMode = getUiTransportMode(configFields.transport)

  function onConnectionModeChange(mode: "stdio" | "http") {
    setConfigFields({
      ...configFields,
      transport: resolveHttpTransport(mode, configFields.transport),
      ...(mode === "http" ? { envText: "" } : {}),
    })
  }

  return (
    <form className={cn("@container flex min-h-0 flex-1 flex-col", className)} onSubmit={onSubmit}>
      <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,1fr)_minmax(240px,20rem)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
        <ScrollPane className="min-h-0 @min-[640px]:min-h-0">
          <FieldGroup className="gap-5 pr-1">
            <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
              <Field>
                <FieldLabel htmlFor="name">{t("name")}</FieldLabel>
                <Input
                  id="name"
                  name="name"
                  value={serviceName}
                  placeholder="github"
                  required
                  autoFocus
                  onChange={(event) => setServiceName(event.target.value)}
                />
              </Field>

              <Field className="sm:min-w-[10rem]">
                <FieldLabel>{t("transport")}</FieldLabel>
                <Tabs
                  value={connectionMode}
                  onValueChange={(value) => onConnectionModeChange(value as "stdio" | "http")}
                >
                  <TabsList className="grid h-9 w-full grid-cols-2">
                    <TabsTrigger value="stdio">stdio</TabsTrigger>
                    <TabsTrigger value="http">http</TabsTrigger>
                  </TabsList>
                </Tabs>
              </Field>
            </div>

            <ServiceConfigFormFields
              fields={configFields}
              onFieldsChange={setConfigFields}
              scopeExtra={
                <FieldCollapsible title={t("scope")}>
                  <Field>
                    <InputGroup>
                      <InputGroupInput
                        id="agentId"
                        name="agentId"
                        value={agentId}
                        placeholder={scope === "agent" ? "agent-a" : t("store")}
                        disabled={scope !== "agent"}
                        required={scope === "agent"}
                        list={scope === "agent" && agentIds.length ? "add-service-agent-options" : undefined}
                        onChange={(event) => setAgentId(event.target.value)}
                      />
                      {agentIds.length ? (
                        <datalist id="add-service-agent-options">
                          {agentIds.map((id) => (
                            <option key={id} value={id} />
                          ))}
                        </datalist>
                      ) : null}
                      <InputGroupAddon align="inline-end" className="px-2">
                        <label className="flex cursor-pointer items-center gap-2 text-sm font-medium text-foreground select-none">
                          <input
                            type="checkbox"
                            className="size-4 shrink-0 rounded border border-input accent-primary"
                            checked={scope === "agent"}
                            onChange={(event) => {
                              const useAgent = event.target.checked
                              setScope(useAgent ? "agent" : "store")
                              if (!useAgent) {
                                setAgentId("")
                              }
                            }}
                          />
                          {t("agent")}
                        </label>
                      </InputGroupAddon>
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpScope")}</FieldDescription>
                  </Field>
                </FieldCollapsible>
              }
            />

            <FieldCollapsible title={t("lifecyclePolicy")}>
              <div className="grid gap-4 sm:grid-cols-2">
                <ServiceStartupPolicySelect value={startupPolicy} onChange={setStartupPolicy} />
                <ServiceRestartPolicySelect value={restartPolicy} onChange={setRestartPolicy} />
              </div>
            </FieldCollapsible>
          </FieldGroup>
        </ScrollPane>

        <AddServicePlaygroundAside
          agentId={agentId}
          className="min-h-0 max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none"
          fields={configFields}
          name={serviceName}
          previewFormat={previewFormat}
          scope={scope}
          onFieldsChange={setConfigFields}
          onNameChange={setServiceName}
          onPreviewFormatChange={setPreviewFormat}
        />
      </div>

      {showActions ? (
        <div className="shrink-0 border-t px-5 py-3.5">
          {onCancel ? (
            <DialogFormFooter
              onCancel={onCancel}
              submitLabel={submitting ? t("adding") : t("navAddService")}
              submitting={submitting}
            />
          ) : (
            <div className="flex justify-end">
              <Button type="submit" disabled={submitting}>
                {submitting ? <Spinner data-icon="inline-start" /> : null}
                {submitting ? t("adding") : t("navAddService")}
              </Button>
            </div>
          )}
        </div>
      ) : null}
    </form>
  )
}
