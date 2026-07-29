import { AddServicePlaygroundAside } from "@/features/services/add-service-playground-aside"
import { ServiceConfigFormFields } from "@/features/services/service-config-form-fields"
import { getUiTransportMode } from "@/features/services/service-config-draft"
import { useEditServiceForm } from "@/features/services/use-edit-service-form"
import { DialogFormFooter } from "@/components/shared/dialog-form"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

export function EditServiceForm({
  className,
  onCancel,
  onUpdated,
  service,
}: {
  className?: string
  onCancel: () => void
  onUpdated: () => Promise<void>
  service: ServiceInstance
}) {
  const { t } = useI18n()
  const {
    configFields,
    onSubmit,
    previewFormat,
    setConfigFields,
    setPreviewFormat,
    submitting,
  } = useEditServiceForm({
    onCancel,
    onUpdated,
    service,
  })

  const scopeLabel =
    service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`
  const transportLabel = getUiTransportMode(configFields.transport)

  return (
    <form className={cn("@container flex min-h-0 flex-1 flex-col", className)} onSubmit={onSubmit}>
      <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,25rem)_minmax(280px,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
        <ScrollPane className="min-h-0 @min-[640px]:min-h-0">
          <FieldGroup className="gap-5 pr-1">
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel>{t("name")}</FieldLabel>
                <Input value={service.service_name} readOnly />
              </Field>
              <Field>
                <FieldLabel>{t("scope")}</FieldLabel>
                <Input value={scopeLabel} readOnly />
              </Field>
            </div>

            <Field>
              <FieldLabel>{t("transport")}</FieldLabel>
              <Input value={transportLabel} readOnly />
            </Field>

            <ServiceConfigFormFields fields={configFields} onFieldsChange={setConfigFields} />
          </FieldGroup>
        </ScrollPane>

        <AddServicePlaygroundAside
          agentId={service.scope.type === "agent" ? service.scope.agent_id : ""}
          className="min-h-0 max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none"
          fields={configFields}
          name={service.service_name}
          previewFormat={previewFormat}
          scope={service.scope.type === "store" ? "store" : "agent"}
          showCli={false}
          onFieldsChange={setConfigFields}
          onPreviewFormatChange={setPreviewFormat}
        />
      </div>

      <div className="shrink-0 border-t px-5 py-3.5">
        <DialogFormFooter
          onCancel={onCancel}
          submitLabel={submitting ? t("saving") : t("save")}
          submitting={submitting}
        />
      </div>
    </form>
  )
}
