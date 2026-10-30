import { useState } from "react"
import {
  EyeIcon,
  LayersIcon,
  PencilIcon,
  RotateCwIcon,
  Trash2Icon,
} from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SERVICE_STATUS_LABEL_KEYS } from "@/components/shared/service-status-badge"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ServiceAuthConnectionFields } from "@/features/services/service-auth-panel"
import {
  isServiceRunning,
  ServiceConnectionButton,
} from "@/features/services/service-connection-button"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { ServiceDetailField } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { formatServiceLaunchLine, getServiceEndpointLabel } from "@/lib/service-info"
import { cn } from "@/lib/utils"

type DetailAsideTab = "actions" | "detail" | "connection" | "mcp"

export function ServiceCardDialog({
  busy,
  onAddScope,
  onConnect,
  onDelete,
  onDisconnect,
  onEdit,
  onOpen,
  onOpenChange,
  onRestart,
  open,
  service,
}: {
  busy: string | null
  onAddScope: (service: ServiceInstance) => void
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onEdit: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onOpenChange: (open: boolean) => void
  onRestart: (service: ServiceInstance) => void
  open: boolean
  service: ServiceInstance | null
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<DetailAsideTab>("actions")

  if (!service) return null

  const connected = isServiceRunning(service.state)
  const status = deriveServiceDisplayStatus(service.state)
  const description = String(service.effective_config?.description || "").trim()
  const transport = service.transport
  const endpoint = getServiceEndpointLabel(service)
  const launchLine = formatServiceLaunchLine(service)
  const configArgs = Array.isArray(service.effective_config?.args)
    ? service.effective_config.args.map(String).join(" ")
    : null
  const scopeLabel =
    service.scope.type === "store"
      ? t("store")
      : service.scope.type === "agent"
        ? `${t("agent")} ${service.scope.agent_id}`
        : service.scope.type
  const capabilities = service.mcp?.capabilities
  const capabilityLabels = capabilities
    ? [
        capabilities.tools && t("tools"),
        capabilities.resources && t("resources"),
        capabilities.resourcesSubscribe && t("resourceSubscriptions"),
        capabilities.prompts && t("prompts"),
        capabilities.completions && t("completions"),
        capabilities.tasks && t("tasks"),
      ].filter((label): label is string => Boolean(label))
    : []
  const hasMcp = Boolean(service.mcp)
  const resolvedTab = hasMcp || activeTab !== "mcp" ? activeTab : "actions"

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

        <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,25rem)_minmax(280px,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
          <ScrollPane className="min-h-0 @min-[640px]:min-h-0">
            <div className="flex flex-col items-center gap-5 pr-1 text-center">
              <div className="flex w-full min-w-0 flex-col items-center gap-3">
                <span
                  aria-hidden
                  className="flex size-16 shrink-0 items-center justify-center rounded-2xl border border-border/50 bg-muted/60 text-muted-foreground"
                >
                  <DefaultServiceIcon className="size-8" />
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
                <ServiceDetailField label={t("agentScope")} value={scopeLabel} />
                <ServiceDetailField
                  label={t("transport")}
                  value={configArgs ? `${transport} · ${configArgs}` : transport}
                />
                <ServiceDetailField label={t("endpoint")} value={endpoint} />
                {description ? (
                  <ServiceDetailField
                    label={t("description")}
                    value={description}
                    mono={false}
                  />
                ) : null}
              </dl>
            </div>
          </ScrollPane>

          <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none">
            <Tabs
              value={resolvedTab}
              onValueChange={(value) => setActiveTab(value as DetailAsideTab)}
              className="flex min-h-0 flex-1 flex-col gap-3 @min-[640px]:gap-4"
            >
              <TabsList variant="line" className="h-9 w-full shrink-0 justify-start overflow-x-auto">
                <TabsTrigger value="actions">{t("actions")}</TabsTrigger>
                <TabsTrigger value="detail">{t("serviceDetail")}</TabsTrigger>
                <TabsTrigger value="connection">{t("connection")}</TabsTrigger>
                {hasMcp ? <TabsTrigger value="mcp">{t("mcpServer")}</TabsTrigger> : null}
              </TabsList>

              <TabsContent value="actions" className="min-h-0 flex-1 overflow-hidden">
                <ScrollPane className="h-full min-h-0">
                  <div className="grid gap-2 pr-1">
                    <ServiceConnectionButton
                      busy={busy}
                      className="w-full justify-start"
                      instanceId={service.instance_id}
                      state={service.state}
                      size="default"
                      variant={connected ? "outline" : "default"}
                      onConnect={() => closeThen(() => onConnect(service))}
                      onDisconnect={() => closeThen(() => onDisconnect(service))}
                    />
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      disabled={Boolean(busy) || !connected}
                      onClick={() => closeThen(() => onRestart(service))}
                    >
                      <RotateCwIcon data-icon="inline-start" />
                      {t("restart")}
                    </Button>
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      onClick={() => closeThen(() => onOpen(service))}
                    >
                      <EyeIcon data-icon="inline-start" />
                      {t("detail")}
                    </Button>
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      onClick={() => closeThen(() => onEdit(service))}
                    >
                      <PencilIcon data-icon="inline-start" />
                      {t("edit")}
                    </Button>
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      disabled={Boolean(busy)}
                      onClick={() => closeThen(() => onAddScope(service))}
                    >
                      <LayersIcon data-icon="inline-start" />
                      {t("addScope")}
                    </Button>
                    <Button
                      variant="destructive"
                      className="w-full justify-start"
                      disabled={Boolean(busy)}
                      onClick={() => closeThen(() => onDelete(service))}
                    >
                      <Trash2Icon data-icon="inline-start" />
                      {t("delete")}
                    </Button>
                  </div>
                </ScrollPane>
              </TabsContent>

              <TabsContent value="detail" className="min-h-0 flex-1 overflow-hidden">
                <ScrollPane className="h-full min-h-0">
                  <dl className="grid gap-3 pr-1 @min-[32rem]:grid-cols-2">
                    <ServiceDetailField label={t("name")} value={service.service_name} />
                    <ServiceDetailField label={t("agentScope")} value={scopeLabel} />
                    <ServiceDetailField label="Instance ID" value={service.instance_id} span />
                    {description ? (
                      <ServiceDetailField
                        label={t("description")}
                        value={description}
                        mono={false}
                        span
                      />
                    ) : null}
                  </dl>
                </ScrollPane>
              </TabsContent>

              <TabsContent value="connection" className="min-h-0 flex-1 overflow-hidden">
                <ScrollPane className="h-full min-h-0">
                  <div className="pr-1">
                    <ServiceAuthConnectionFields
                      service={service}
                      render={({ statusField, actions }) => (
                        <div className="flex flex-col gap-4">
                          <dl className="grid gap-3 @min-[32rem]:grid-cols-2">
                            <ServiceDetailField
                              label={t("transport")}
                              value={configArgs ? `${transport} · ${configArgs}` : transport}
                            />
                            {statusField}
                            <ServiceDetailField label={t("endpoint")} value={endpoint} span />
                            <ServiceDetailField label={t("launch")} value={launchLine} span />
                          </dl>
                          {actions}
                        </div>
                      )}
                    />
                  </div>
                </ScrollPane>
              </TabsContent>

              {hasMcp && service.mcp ? (
                <TabsContent value="mcp" className="min-h-0 flex-1 overflow-hidden">
                  <ScrollPane className="h-full min-h-0">
                    <dl className="grid gap-3 pr-1 @min-[32rem]:grid-cols-2">
                      <ServiceDetailField
                        label={t("serverImplementation")}
                        value={`${service.mcp.serverInfo.title || service.mcp.serverInfo.name} · ${service.mcp.serverInfo.version}`}
                      />
                      <ServiceDetailField
                        label={t("protocolVersion")}
                        value={service.mcp.protocolVersion}
                      />
                      {capabilityLabels.length ? (
                        <div className="@min-[32rem]:col-span-2">
                          <dt className="font-mono text-xs uppercase text-muted-foreground">
                            {t("capabilities")}
                          </dt>
                          <dd className="mt-1 flex flex-wrap gap-2">
                            {capabilityLabels.map((label) => (
                              <Badge key={label} variant="outline">
                                {label}
                              </Badge>
                            ))}
                          </dd>
                        </div>
                      ) : null}
                      {service.mcp.instructions ? (
                        <ServiceDetailField
                          label={t("serverInstructions")}
                          value={
                            <span className="whitespace-pre-wrap font-sans text-sm">
                              {service.mcp.instructions}
                            </span>
                          }
                          mono={false}
                          span
                        />
                      ) : null}
                    </dl>
                  </ScrollPane>
                </TabsContent>
              ) : null}
            </Tabs>
          </aside>
        </div>
      </DialogContent>
    </Dialog>
  )
}
