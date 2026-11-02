import { useState, type ReactNode } from "react"
import {
  EyeIcon,
  LayersIcon,
  RotateCwIcon,
  Trash2Icon,
} from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { DialogFormFooter } from "@/components/shared/dialog-form"
import { McpIcon } from "@/components/shared/mcp-icon"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
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
import {
  ServiceConfigDescriptionPane,
  ServiceConfigHandshakePane,
  ServiceConfigPreviewPane,
} from "@/features/services/add-service-playground-aside"
import { useServiceDetailQuery } from "@/features/services/queries"
import { ServiceConfigFormFields } from "@/features/services/service-config-form-fields"
import {
  isServiceRunning,
  ServiceConnectionButton,
} from "@/features/services/service-connection-button"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { ServiceDetailField } from "@/features/services/service-panels"
import { useEditServiceForm } from "@/features/services/use-edit-service-form"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { getServiceEndpointLabel } from "@/lib/service-info"
import { cn } from "@/lib/utils"

type DetailAsideTab = "edit" | "preview" | "profile" | "scope"

const EDIT_TABS = new Set<DetailAsideTab>(["edit", "profile"])

function QuietScroll({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div
      className={cn(
        "min-h-0 overflow-x-hidden overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden",
        className,
      )}
    >
      {children}
    </div>
  )
}

export function ServiceCardDialog({
  busy,
  onAddScope,
  onConnect,
  onDelete,
  onDisconnect,
  onOpen,
  onOpenChange,
  onRestart,
  onUpdated,
  open,
  service: listService,
}: {
  busy: string | null
  onAddScope: (service: ServiceInstance) => void
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onOpenChange: (open: boolean) => void
  onRestart: (service: ServiceInstance) => void
  onUpdated: () => Promise<void>
  open: boolean
  service: ServiceInstance | null
}) {
  if (!listService) return null

  return (
    <ServiceCardDialogBody
      busy={busy}
      listService={listService}
      open={open}
      onAddScope={onAddScope}
      onConnect={onConnect}
      onDelete={onDelete}
      onDisconnect={onDisconnect}
      onOpen={onOpen}
      onOpenChange={onOpenChange}
      onRestart={onRestart}
      onUpdated={onUpdated}
    />
  )
}

function ServiceCardDialogBody({
  busy,
  listService,
  onAddScope,
  onConnect,
  onDelete,
  onDisconnect,
  onOpen,
  onOpenChange,
  onRestart,
  onUpdated,
  open,
}: {
  busy: string | null
  listService: ServiceInstance
  onAddScope: (service: ServiceInstance) => void
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onOpenChange: (open: boolean) => void
  onRestart: (service: ServiceInstance) => void
  onUpdated: () => Promise<void>
  open: boolean
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<DetailAsideTab>("preview")
  const detailQuery = useServiceDetailQuery(listService, open)
  const service = detailQuery.data || listService
  const {
    configFields,
    onSubmit,
    previewFormat,
    setConfigFields,
    setPreviewFormat,
    submitting,
  } = useEditServiceForm({
    onCancel: () => setActiveTab("preview"),
    onUpdated,
    service,
  })

  const connected = isServiceRunning(service.state)
  const status = deriveServiceDisplayStatus(service.state)
  const transport = service.transport
  const endpoint = getServiceEndpointLabel(service)
  const scopeLabel =
    service.scope.type === "store"
      ? t("store")
      : `${t("agent")} ${service.scope.agent_id}`
  const isEditTab = EDIT_TABS.has(activeTab)
  const editScope = service.scope.type === "store" ? "store" : "agent"
  const editAgentId = service.scope.type === "agent" ? service.scope.agent_id : ""

  function closeThen(action: () => void) {
    onOpenChange(false)
    action()
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[68dvh] w-[min(74vw,68rem)]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-4 @min-[640px]:px-5">
          <DialogTitle className="truncate font-mono">{service.service_name}</DialogTitle>
          <DialogDescription className="sr-only">
            {t("serviceListMoreActionsDescription", { name: service.service_name })}
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex min-h-0 flex-1 flex-col"
          onSubmit={isEditTab ? onSubmit : (event) => event.preventDefault()}
        >
          <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,25rem)_minmax(280px,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
            <QuietScroll className="min-h-0 @min-[640px]:min-h-0">
              <div className="flex flex-col items-center gap-5 pt-3 text-center">
                <div className="flex w-full min-w-0 flex-col items-center gap-3">
                  <span
                    aria-hidden
                    className="flex size-16 shrink-0 items-center justify-center rounded-2xl border border-border/50 bg-muted/60 text-muted-foreground"
                  >
                    <McpIcon
                      icons={service.mcp?.serverInfo?.icons}
                      fallback={<DefaultServiceIcon className="size-8" />}
                      className="size-8"
                    />
                  </span>
                  <div className="min-w-0 w-full">
                    <p className="truncate font-mono text-base font-semibold">{service.service_name}</p>
                    <p
                      className={cn(
                        "mt-1 text-xs",
                        connected ? "text-emerald-600 dark:text-emerald-500" : "text-muted-foreground",
                      )}
                    >
                      {t(SERVICE_STATUS_LABEL_KEYS[status])}
                    </p>
                  </div>
                </div>

                <dl className="grid w-full gap-3 text-start">
                  {service.mcp ? (
                    <ServiceDetailField
                      label={t("serverImplementation")}
                      value={`${service.mcp.serverInfo.title || service.mcp.serverInfo.name} · ${service.mcp.serverInfo.version}`}
                    />
                  ) : null}
                  {service.mcp ? (
                    <div className="grid grid-cols-2 gap-3">
                      <ServiceDetailField
                        label={t("protocolVersion")}
                        value={service.mcp.protocolVersion}
                      />
                      <ServiceDetailField label={t("transport")} value={transport} />
                    </div>
                  ) : (
                    <ServiceDetailField label={t("transport")} value={transport} />
                  )}
                  <ServiceDetailField label={t("endpoint")} value={endpoint} />
                </dl>
              </div>
            </QuietScroll>

            <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none">
              <Tabs
                value={activeTab}
                onValueChange={(value) => setActiveTab(value as DetailAsideTab)}
                className="flex min-h-0 flex-1 flex-col gap-3 @min-[640px]:gap-4"
              >
                <TabsList
                  variant="line"
                  className="h-9 w-fit max-w-full shrink-0 justify-start overflow-x-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
                >
                  <TabsTrigger value="preview" className="flex-none">
                    Preview
                  </TabsTrigger>
                  <TabsTrigger value="scope" className="flex-none">
                    {t("scope")}
                  </TabsTrigger>
                  <TabsTrigger value="edit" className="flex-none">
                    {t("edit")}
                  </TabsTrigger>
                  <TabsTrigger value="profile" className="flex-none">
                    {t("profile")}
                  </TabsTrigger>
                </TabsList>

                <TabsContent value="preview" className="flex min-h-0 flex-1 flex-col overflow-hidden">
                  <ServiceConfigPreviewPane
                    agentId={editAgentId}
                    className="h-full [&_textarea]:[scrollbar-width:none] [&_textarea]:[-ms-overflow-style:none] [&_textarea::-webkit-scrollbar]:hidden"
                    fields={configFields}
                    name={service.service_name}
                    previewFormat={previewFormat}
                    scope={editScope}
                    showCli={false}
                    onFieldsChange={setConfigFields}
                    onPreviewFormatChange={setPreviewFormat}
                  />
                </TabsContent>

                <TabsContent value="scope" className="min-h-0 flex-1 overflow-hidden">
                  <QuietScroll className="h-full">
                    <div className="flex flex-col gap-4">
                      <dl className="grid gap-3">
                        <ServiceDetailField label={t("agentScope")} value={scopeLabel} />
                      </dl>
                      <Button
                        type="button"
                        variant="outline"
                        className="w-full justify-start"
                        disabled={Boolean(busy)}
                        onClick={() => closeThen(() => onAddScope(service))}
                      >
                        <LayersIcon data-icon="inline-start" />
                        {t("addScope")}
                      </Button>
                    </div>
                  </QuietScroll>
                </TabsContent>

                <TabsContent value="edit" className="min-h-0 flex-1 overflow-hidden">
                  <QuietScroll className="h-full">
                    <ServiceConfigFormFields
                      fields={configFields}
                      showOauth={false}
                      onFieldsChange={setConfigFields}
                    />
                  </QuietScroll>
                </TabsContent>

                <TabsContent value="profile" className="min-h-0 flex-1 overflow-hidden">
                  <QuietScroll className="h-full">
                    <div className="flex flex-col gap-5">
                      <ServiceConfigDescriptionPane
                        fields={configFields}
                        onFieldsChange={setConfigFields}
                      />
                      <ServiceConfigHandshakePane
                        fields={configFields}
                        onFieldsChange={setConfigFields}
                      />
                    </div>
                  </QuietScroll>
                </TabsContent>
              </Tabs>
            </aside>
          </div>

          <div className="shrink-0 border-t px-5 py-3.5">
            {isEditTab ? (
              <DialogFormFooter
                onCancel={() => setActiveTab("preview")}
                submitLabel={submitting ? t("saving") : t("save")}
                submitting={submitting}
              />
            ) : (
              <DialogFooter className="sm:justify-between">
                <div className="flex flex-col-reverse gap-2 sm:flex-row">
                  <Button
                    type="button"
                    variant="outline"
                    disabled={Boolean(busy) || !connected}
                    onClick={() => closeThen(() => onRestart(service))}
                  >
                    <RotateCwIcon data-icon="inline-start" />
                    {t("restart")}
                  </Button>
                  <Button
                    type="button"
                    variant="destructive"
                    disabled={Boolean(busy)}
                    onClick={() => closeThen(() => onDelete(service))}
                  >
                    <Trash2Icon data-icon="inline-start" />
                    {t("delete")}
                  </Button>
                </div>
                <div className="flex flex-col-reverse gap-2 sm:flex-row">
                  <ServiceConnectionButton
                    busy={busy}
                    instanceId={service.instance_id}
                    state={service.state}
                    size="default"
                    variant="outline"
                    onConnect={() => closeThen(() => onConnect(service))}
                    onDisconnect={() => closeThen(() => onDisconnect(service))}
                  />
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => closeThen(() => onOpen(service))}
                  >
                    <EyeIcon data-icon="inline-start" />
                    {t("detail")}
                  </Button>
                </div>
              </DialogFooter>
            )}
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
