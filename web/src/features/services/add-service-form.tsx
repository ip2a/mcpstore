import { useScopeServicesQuery } from "@/features/agents/queries"
import { useAddServiceForm } from "@/features/services/use-add-service-form"
import { AddServicePlaygroundAside } from "@/features/services/add-service-playground-aside"
import { ServiceConfigFormFields } from "@/features/services/service-config-form-fields"
import {
  getUiTransportMode,
  resolveHttpTransport,
  serviceInstanceToFields,
} from "@/features/services/service-config-draft"
import { DialogFormFooter } from "@/components/shared/dialog-form"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import type { AgentItem, ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

export function AddServiceForm({
  agents: _agents,
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
  const storeServicesQuery = useScopeServicesQuery({ type: "store" })
  const existingServices = storeServicesQuery.data ?? []
  const {
    agentId,
    configFields,
    onSubmit,
    previewFormat,
    restartPolicy,
    scope,
    serviceName,
    setConfigFields,
    setPreviewFormat,
    setRestartPolicy,
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

  function fillFromExisting(service: ServiceInstance) {
    setServiceName(service.service_name)
    setConfigFields(serviceInstanceToFields(service))
  }

  const fillFromExistingButton = (
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="outline" disabled={submitting}>
          {t("fillFromExistingService")}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-72 min-w-48 overflow-y-auto">
        {storeServicesQuery.isPending ? (
          <DropdownMenuItem disabled>{t("loading")}</DropdownMenuItem>
        ) : existingServices.length === 0 ? (
          <DropdownMenuItem disabled>{t("noServices")}</DropdownMenuItem>
        ) : (
          existingServices.map((service) => (
            <DropdownMenuItem
              key={service.instance_id}
              onClick={() => fillFromExisting(service)}
            >
              {service.service_name}
            </DropdownMenuItem>
          ))
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )

  return (
    <form className={cn("@container flex min-h-0 flex-1 flex-col", className)} onSubmit={onSubmit}>
      <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,25rem)_minmax(280px,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
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

            <ServiceConfigFormFields fields={configFields} onFieldsChange={setConfigFields} />
          </FieldGroup>
        </ScrollPane>

        <AddServicePlaygroundAside
          agentId={agentId}
          className="min-h-0 max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none"
          fields={configFields}
          name={serviceName}
          previewFormat={previewFormat}
          restartPolicy={restartPolicy}
          scope={scope}
          showLifecycleTab
          startupPolicy={startupPolicy}
          onFieldsChange={setConfigFields}
          onNameChange={setServiceName}
          onPreviewFormatChange={setPreviewFormat}
          onRestartPolicyChange={setRestartPolicy}
          onStartupPolicyChange={setStartupPolicy}
        />
      </div>

      {showActions ? (
        <div className="shrink-0 border-t px-5 py-3.5">
          {onCancel ? (
            <DialogFormFooter
              start={fillFromExistingButton}
              onCancel={onCancel}
              submitLabel={submitting ? t("adding") : t("navAddService")}
              submitting={submitting}
            />
          ) : (
            <div className="flex items-center justify-between gap-2">
              {fillFromExistingButton}
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
