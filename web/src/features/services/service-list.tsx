import { useState } from "react"
import { LayersIcon, LinkIcon, MoreHorizontalIcon, RotateCwIcon, Trash2Icon, UnlinkIcon } from "lucide-react"

import { EntityRow } from "@/components/shared/entity-row"
import { ServiceRowMeta } from "@/components/shared/service-row-meta"
import { ServiceConnectionMark } from "@/components/shared/service-status-badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Spinner } from "@/components/ui/spinner"
import {
  isServiceRunning,
  isServiceStarting,
  isServiceDisconnecting,
  ServiceConnectionButtonForEntry,
} from "@/features/services/service-connection-button"
import { AddServiceScopeDialog } from "@/features/services/add-service-scope-dialog"
import { EditServiceDialog } from "@/features/services/edit-service-dialog"
import { useI18n } from "@/lib/i18n-context"
import { getServiceEndpointLabel } from "@/lib/service-info"
import type { AgentItem, ServiceInstance } from "@/lib/api"

function ServiceMoreActionsDialog({
  busy,
  onConnect,
  onDelete,
  onDisconnect,
  onAddScope,
  onOpenChange,
  onRestart,
  service,
}: {
  busy: string | null
  onConnect: (service: ServiceInstance) => void
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onAddScope: (service: ServiceInstance) => void
  onOpenChange: (open: boolean) => void
  onRestart: (service: ServiceInstance) => void
  service: ServiceInstance | null
}) {
  const { t } = useI18n()
  const connected = service ? isServiceRunning(service.state) : false
  const connecting = service ? isServiceStarting(service.state, busy, service.instance_id) : false
  const disconnecting = service ? isServiceDisconnecting(busy, service.instance_id) : false

  return (
    <Dialog open={Boolean(service)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("serviceListMoreActions")}</DialogTitle>
          <DialogDescription className="truncate">
            {service ? t("serviceListMoreActionsDescription", { name: service.service_name }) : null}
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          {connected ? (
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant="outline"
                disabled={Boolean(busy) || !service}
                onClick={() => {
                  if (!service) return
                  onOpenChange(false)
                  onDisconnect(service)
                }}
              >
                {disconnecting ? <Spinner data-icon="inline-start" /> : <UnlinkIcon data-icon="inline-start" />}
                {disconnecting ? t("disconnecting") : t("disconnect")}
              </Button>
              <Button
                variant="outline"
                disabled={Boolean(busy) || !service}
                onClick={() => {
                  if (!service) return
                  onOpenChange(false)
                  onRestart(service)
                }}
              >
                <RotateCwIcon data-icon="inline-start" />
                {t("reconnect")}
              </Button>
            </div>
          ) : connecting ? (
            <Button variant="outline" disabled>
              <Spinner data-icon="inline-start" />
              {t("connecting")}
            </Button>
          ) : (
            <Button
              disabled={Boolean(busy) || !service}
              onClick={() => {
                if (!service) return
                onOpenChange(false)
                onConnect(service)
              }}
            >
              <LinkIcon data-icon="inline-start" />
              {t("connect")}
            </Button>
          )}
          <div className="grid grid-cols-2 gap-2">
            <Button
              variant="outline"
              disabled={Boolean(busy) || !service || !connected}
              onClick={() => {
                if (!service) return
                onOpenChange(false)
                onRestart(service)
              }}
            >
              <RotateCwIcon data-icon="inline-start" />
              {t("restart")}
            </Button>
            <Button
              variant="outline"
              disabled={Boolean(busy) || !service}
              onClick={() => {
                if (!service) return
                onOpenChange(false)
                onAddScope(service)
              }}
            >
              <LayersIcon data-icon="inline-start" />
              {t("addScope")}
            </Button>
          </div>
          <div className="border-t pt-3">
            <Button
              variant="destructive"
              className="w-full"
              disabled={Boolean(busy) || !service}
              onClick={() => {
                if (!service) return
                onOpenChange(false)
                onDelete(service)
              }}
            >
              <Trash2Icon data-icon="inline-start" />
              {t("delete")}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function ServiceRow({
  busy,
  onAddScope,
  onConnect,
  onDisconnect,
  onEdit,
  onMore,
  onOpen,
  service,
}: {
  busy: string | null
  onAddScope: (service: ServiceInstance) => void
  onConnect: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onEdit: (service: ServiceInstance) => void
  onMore: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  service: ServiceInstance
}) {
  const { t } = useI18n()
  const toolCount = service.tools?.length || 0

  return (
    <EntityRow
      variant="inline"
      className="min-h-14 cursor-pointer py-2.5 hover:bg-muted/60"
      tabIndex={0}
      onClick={() => onOpen(service)}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onOpen(service)
        }
      }}
      actions={
        <>
          <ServiceConnectionButtonForEntry busy={busy} service={service} onConnect={onConnect} onDisconnect={onDisconnect} />
          <Button variant="outline" size="sm" onClick={() => onOpen(service)}>
            {t("detail")}
          </Button>
          <Button variant="outline" size="sm" onClick={() => onEdit(service)}>
            {t("edit")}
          </Button>
          <Button variant="outline" size="sm" aria-label={t("serviceListMoreActionsFor", { name: service.service_name })} onClick={() => onMore(service)}>
            <MoreHorizontalIcon data-icon="inline-start" />
            {t("more")}
          </Button>
        </>
      }
      actionsProps={{ onClick: (event) => event.stopPropagation() }}
    >
      <div className="min-w-0">
        <div className="flex min-w-0 flex-nowrap items-center gap-x-2 text-base">
          <span className="min-w-0 truncate font-semibold">{service.service_name}</span>
          <ServiceConnectionMark state={service.state} />
          <span className="min-w-0 truncate text-sm text-muted-foreground" title={getServiceEndpointLabel(service)}>
            {getServiceEndpointLabel(service)}
          </span>
        </div>
        <ServiceRowMeta service={service} showScope toolCount={toolCount} />
      </div>
    </EntityRow>
  )
}

export function ServiceList(props: {
  agents: AgentItem[]
  allServices: ServiceInstance[]
  services: ServiceInstance[]
  busy: string | null
  onConnect: (service: ServiceInstance) => void
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
  onDelete: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
}) {
  const [moreService, setMoreService] = useState<ServiceInstance | null>(null)
  const [editService, setEditService] = useState<ServiceInstance | null>(null)
  const [scopeService, setScopeService] = useState<ServiceInstance | null>(null)

  return (
    <>
      <div>
        {props.services.map((service) => (
          <ServiceRow
            key={service.instance_id}
            busy={props.busy}
            onAddScope={setScopeService}
            onConnect={props.onConnect}
            onDisconnect={props.onDisconnect}
            onEdit={setEditService}
            onMore={setMoreService}
            onOpen={props.onOpen}
            service={service}
          />
        ))}
      </div>
      <EditServiceDialog
        open={Boolean(editService)}
        service={editService}
        onOpenChange={(open) => {
          if (!open) setEditService(null)
        }}
        onUpdated={async () => {
          await props.onRefresh()
        }}
      />
      <ServiceMoreActionsDialog
        busy={props.busy}
        service={moreService}
        onOpenChange={(open) => {
          if (!open) setMoreService(null)
        }}
        onConnect={props.onConnect}
        onAddScope={setScopeService}
        onDelete={props.onDelete}
        onDisconnect={props.onDisconnect}
        onRestart={props.onRestart}
      />
      <AddServiceScopeDialog
        agents={props.agents}
        allServices={props.allServices}
        busy={props.busy}
        service={scopeService}
        onOpenChange={(open) => {
          if (!open) setScopeService(null)
        }}
        onDeclareScope={props.onDeclareScope}
      />
    </>
  )
}
