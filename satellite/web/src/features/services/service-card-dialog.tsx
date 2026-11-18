import { useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowDownIcon, ArrowUpIcon, EyeIcon, RotateCwIcon, Trash2Icon } from "lucide-react"

import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { McpIcon } from "@/components/shared/mcp-icon"
import { PageSkeleton } from "@/components/shared/page-states"
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
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  ServiceConfigDescriptionPane,
  ServiceConfigHandshakePane,
  ServiceConfigPreviewPane,
} from "@/features/services/add-service-playground-aside"
import { useServiceDetailQuery } from "@/features/services/queries"
import { useServiceOverrideQuery } from "@/features/overrides/queries"
import {
  ServiceOverridePane,
  type ServiceOverridePaneHandle,
} from "@/features/services/service-override-pane"
import { ServiceConfigFormFields } from "@/features/services/service-config-form-fields"
import {
  isServiceRunning,
  isServiceStarting,
  ServiceConnectionButton,
} from "@/features/services/service-connection-button"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import { useEditServiceForm } from "@/features/services/use-edit-service-form"
import { useServiceCardSessionFrames } from "@/features/services/use-service-card-session-frames"
import { FrameDetailPane } from "@/features/traces/frame-session-panes"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import type { FrameRecord, ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

export type ServiceCardAsideTab = "edit" | "preview" | "profile" | "gateway" | "sessions"

type DetailAsideTab = ServiceCardAsideTab

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

/** Map jsonrpc id → request method so bare responses can show whose reply they are. */
function methodByJsonrpcId(frames: FrameRecord[]): Map<string, string> {
  const map = new Map<string, string>()
  for (const frame of frames) {
    if (frame.kind !== "request" || !frame.jsonrpc_id || !frame.method) continue
    map.set(frame.jsonrpc_id, frame.method)
  }
  return map
}

function frameChipTitle(frame: FrameRecord, methods: Map<string, string>): string {
  if (frame.method) return frame.method
  // Responses/errors have no method on the wire — resolve via matching request id.
  if (frame.jsonrpc_id) {
    const paired = methods.get(frame.jsonrpc_id)
    if (paired) return paired
    return `#${frame.jsonrpc_id}`
  }
  return "—"
}

/** Compact frame chip for the card's fixed left rail (~golden-ratio panel). */
function CardSessionFrameChip({
  frame,
  methods,
  selected,
  onSelect,
}: {
  frame: FrameRecord
  methods: Map<string, string>
  selected: boolean
  onSelect: () => void
}) {
  const Arrow = frame.direction === "up" ? ArrowUpIcon : ArrowDownIcon
  const time = new Date(frame.ts_unix_ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
  const title = frameChipTitle(frame, methods)

  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "flex w-full min-w-0 flex-col gap-0.5 rounded-md border px-2.5 py-2 text-left transition-colors",
        selected
          ? "border-border bg-secondary"
          : "border-transparent hover:border-border/60 hover:bg-muted/40",
      )}
    >
      <span className="flex min-w-0 items-center gap-1.5">
        <Arrow
          className={cn(
            "size-3 shrink-0",
            frame.direction === "up" ? "text-blue-500" : "text-emerald-500",
          )}
        />
        <span className="min-w-0 flex-1 truncate font-mono text-xs font-medium">{title}</span>
      </span>
      <span className="flex min-w-0 items-center justify-between gap-2 pl-[1.125rem] text-[10px] text-muted-foreground">
        <span className="tabular-nums">{time}</span>
        <span className="truncate">{frame.kind}</span>
      </span>
    </button>
  )
}

export function ServiceCardDialog({
  busy,
  initialTab = "preview",
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
  initialTab?: DetailAsideTab
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
      key={`${listService.instance_id}:${initialTab}`}
      busy={busy}
      initialTab={initialTab}
      listService={listService}
      open={open}
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
  initialTab,
  listService,
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
  initialTab: DetailAsideTab
  listService: ServiceInstance
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
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<DetailAsideTab>(initialTab)
  const [gatewayBusy, setGatewayBusy] = useState(false)
  /** After Connect, ignore older sessions so the left rail tracks this attempt. */
  const [connectWatchFromMs, setConnectWatchFromMs] = useState(0)
  const overridePaneRef = useRef<ServiceOverridePaneHandle>(null)
  const detailQuery = useServiceDetailQuery(listService, open)
  const service = detailQuery.data || listService
  const isSessionsTab = activeTab === "sessions"
  const connecting = isServiceStarting(service.state, busy, service.instance_id)
  const sessionFrames = useServiceCardSessionFrames(service.instance_id, open && isSessionsTab, {
    followLatest: connecting,
    notBeforeMs: connectWatchFromMs,
  })
  const sessionMethods = useMemo(
    () => methodByJsonrpcId(sessionFrames.frames),
    [sessionFrames.frames],
  )
  const { configFields, onSubmit, previewFormat, setConfigFields, setPreviewFormat, submitting } =
    useEditServiceForm({
      onCancel: () => setActiveTab("preview"),
      onUpdated,
      service,
    })

  useEffect(() => {
    setActiveTab(initialTab)
  }, [initialTab])

  // Once the post-connect session has frames, drop the watermark so later views
  // can still open the latest session normally.
  useEffect(() => {
    if (!connectWatchFromMs || connecting) return
    if (sessionFrames.sessionId) setConnectWatchFromMs(0)
  }, [connectWatchFromMs, connecting, sessionFrames.sessionId])

  const connected = isServiceRunning(service.state)
  const status = deriveServiceDisplayStatus(service.state)
  // Dialog chrome uses list projection first; override query stays for the editor pane.
  const overrideQuery = useServiceOverrideQuery(service.service_name, service.scope)
  const serviceOverride = overrideQuery.data?.override
  const projectedName =
    serviceOverride?.display_name?.trim() ||
    service.display_name?.trim() ||
    service.service_name
  const scopeLabel =
    service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`
  const isEditTab = EDIT_TABS.has(activeTab)
  const isGatewayTab = activeTab === "gateway"
  const showSaveFooter = isEditTab || isGatewayTab
  const editScope = service.scope.type === "store" ? "store" : "agent"
  const editAgentId = service.scope.type === "agent" ? service.scope.agent_id : ""

  function closeThen(action: () => void) {
    onOpenChange(false)
    action()
  }

  function cancelEditing() {
    if (isGatewayTab) overridePaneRef.current?.discard()
    setActiveTab("preview")
  }

  async function onFormSubmit(event: FormEvent<HTMLFormElement>) {
    if (isGatewayTab) {
      event.preventDefault()
      await overridePaneRef.current?.save()
      return
    }
    if (isEditTab) {
      await onSubmit(event)
      return
    }
    event.preventDefault()
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[68dvh] w-[min(90vw,calc(68dvh*1.618))]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-4 @min-[640px]:px-5">
          <DialogTitle className="truncate font-mono">{projectedName}</DialogTitle>
          <DialogDescription className="sr-only">
            {t("serviceListMoreActionsDescription", { name: service.service_name })}
          </DialogDescription>
        </DialogHeader>

        <form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void onFormSubmit(event)}>
          <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 pb-4 pt-0 @min-[640px]:grid-cols-[auto_minmax(0,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:pb-4">
            <QuietScroll className="flex min-h-0 pt-4 @min-[640px]:h-full @min-[640px]:w-auto @min-[640px]:max-w-full @min-[640px]:aspect-[1/1.618]">
              {isSessionsTab ? (
                <div className="flex h-full min-h-0 w-full flex-col gap-1">
                  {sessionFrames.loading ? (
                    <PageSkeleton />
                  ) : sessionFrames.noSession ? (
                    <p className="m-auto px-2 text-center text-xs text-muted-foreground">
                      {connecting || connectWatchFromMs
                        ? t("connecting")
                        : t("gatewayFramesEmptyUpstream")}
                    </p>
                  ) : sessionFrames.frames.length === 0 ? (
                    <p className="m-auto px-2 text-center text-xs text-muted-foreground">
                      {connecting ? t("connecting") : t("gatewayFramesNoFrames")}
                    </p>
                  ) : (
                    sessionFrames.frames.map((frame) => (
                      <CardSessionFrameChip
                        key={frame.id}
                        frame={frame}
                        methods={sessionMethods}
                        selected={frame.id === sessionFrames.selectedId}
                        onSelect={() => sessionFrames.setSelectedId(frame.id)}
                      />
                    ))
                  )}
                </div>
              ) : (
                <button
                  type="button"
                  className="m-auto flex w-full min-w-0 cursor-pointer flex-col items-center gap-3 rounded-lg text-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  aria-label={t("detail")}
                  onClick={() => closeThen(() => onOpen(service))}
                >
                  <span
                    aria-hidden
                    className="flex size-16 shrink-0 items-center justify-center text-muted-foreground"
                  >
                    <McpIcon
                      icons={serviceOverride?.icons ?? service.mcp?.serverInfo?.icons}
                      fallback={<DefaultServiceIcon className="size-16" />}
                      className="size-16"
                    />
                  </span>
                  <div className="min-w-0 w-full">
                    <p className="truncate font-mono text-base font-semibold">{projectedName}</p>
                    {serviceOverride?.description ? (
                      <p className="mt-1 text-xs text-muted-foreground">
                        {serviceOverride.description}
                      </p>
                    ) : null}
                    <p
                      className={cn(
                        "mt-1 text-xs",
                        connected
                          ? "text-emerald-600 dark:text-emerald-500"
                          : "text-muted-foreground",
                      )}
                    >
                      {t(SERVICE_STATUS_LABEL_KEYS[status])}
                    </p>
                  </div>
                </button>
              )}
            </QuietScroll>

            <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden pt-2 @min-[640px]:h-full @min-[640px]:max-h-none">
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
                  <TabsTrigger value="edit" className="flex-none">
                    {t("edit")}
                  </TabsTrigger>
                  <TabsTrigger value="profile" className="flex-none">
                    {t("profile")}
                  </TabsTrigger>
                  <TabsTrigger value="gateway" className="flex-none">
                    {t("gateway")}
                  </TabsTrigger>
                  <TabsTrigger value="sessions" className="flex-none">
                    {t("gatewayFramesTabSessions")}
                  </TabsTrigger>
                </TabsList>

                <TabsContent
                  value="preview"
                  className="flex min-h-0 flex-1 flex-col overflow-hidden"
                >
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
                      <Field
                        orientation="horizontal"
                        className="justify-between [&>[data-slot=field-label]]:flex-none"
                      >
                        <FieldLabel htmlFor="card-service-scope">{t("scope")}</FieldLabel>
                        <Input
                          id="card-service-scope"
                          className="w-40 max-w-[10rem]"
                          value={scopeLabel}
                          readOnly
                        />
                      </Field>
                      <ServiceConfigHandshakePane
                        fields={configFields}
                        onFieldsChange={setConfigFields}
                      />
                      <ServiceConfigDescriptionPane
                        fields={configFields}
                        onFieldsChange={setConfigFields}
                      />
                    </div>
                  </QuietScroll>
                </TabsContent>

                <TabsContent value="gateway" className="min-h-0 flex-1 overflow-hidden">
                  <QuietScroll className="h-full">
                    <ServiceOverridePane
                      ref={overridePaneRef}
                      hideActions
                      serviceName={service.service_name}
                      scope={service.scope}
                      onBusyChange={setGatewayBusy}
                    />
                  </QuietScroll>
                </TabsContent>

                <TabsContent
                  value="sessions"
                  className="flex min-h-0 flex-1 flex-col overflow-hidden"
                >
                  {sessionFrames.loading ? (
                    <PageSkeleton />
                  ) : sessionFrames.selected ? (
                    <FrameDetailPane
                      frame={sessionFrames.selected}
                      insetClassName="px-0 py-0"
                    />
                  ) : (
                    <p className="px-1 py-2 text-sm text-muted-foreground">
                      {sessionFrames.noSession
                        ? t("gatewayFramesEmptyUpstream")
                        : t("gatewayFramesNoFrames")}
                    </p>
                  )}
                </TabsContent>
              </Tabs>
            </aside>
          </div>

          <div className="shrink-0 border-t px-5 py-3.5">
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
              {showSaveFooter ? (
                <div className="flex flex-col-reverse gap-2 sm:flex-row">
                  <Button type="button" variant="outline" onClick={cancelEditing}>
                    {t("cancel")}
                  </Button>
                  <Button
                    type="submit"
                    disabled={isGatewayTab ? gatewayBusy : submitting}
                  >
                    {(isGatewayTab ? gatewayBusy : submitting) ? (
                      <Spinner data-icon="inline-start" />
                    ) : null}
                    {(isGatewayTab ? gatewayBusy : submitting) ? t("saving") : t("save")}
                  </Button>
                </div>
              ) : (
                <div className="flex flex-col-reverse gap-2 sm:flex-row">
                  <ServiceConnectionButton
                    busy={busy}
                    instanceId={service.instance_id}
                    state={service.state}
                    size="default"
                    variant="outline"
                    onConnect={() => {
                      setActiveTab("sessions")
                      setConnectWatchFromMs(Date.now() - 2_000)
                      void queryClient.invalidateQueries({
                        queryKey: [
                          ...queryKeys.gatewayFrameSessions,
                          "by-instance",
                          service.instance_id,
                        ],
                      })
                      onConnect(service)
                    }}
                    onDisconnect={() => {
                      setConnectWatchFromMs(0)
                      onDisconnect(service)
                    }}
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
              )}
            </DialogFooter>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
