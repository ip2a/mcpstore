import { useState } from "react"
import { GripVerticalIcon, LayersIcon, MoreHorizontalIcon, RotateCwIcon, Trash2Icon } from "lucide-react"

import { EntityRow } from "@/components/shared/entity-row"
import { ServiceTransitionIcon } from "@/components/shared/service-transition-icon"
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
                {disconnecting ? <Spinner data-icon="inline-start" /> : <ServiceTransitionIcon direction="disconnect" data-icon="inline-start" />}
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
              <ServiceTransitionIcon direction="connect" data-icon="inline-start" />
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
  dragging,
  dropTarget,
  onAddScope,
  onConnect,
  onDisconnect,
  onDragEnd,
  onDragStart,
  onDropTarget,
  onEdit,
  onMore,
  onMove,
  onOpen,
  service,
}: {
  busy: string | null
  dragging: boolean
  dropTarget: boolean
  onAddScope: (service: ServiceInstance) => void
  onConnect: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  onDragEnd: () => void
  onDragStart: (instanceId: string) => void
  onDropTarget: (instanceId: string) => void
  onEdit: (service: ServiceInstance) => void
  onMore: (service: ServiceInstance) => void
  onMove: (fromId: string, toId: string) => void
  onOpen: (service: ServiceInstance) => void
  service: ServiceInstance
}) {
  const { t } = useI18n()
  const toolCount = service.tools?.length || 0

  return (
    <EntityRow
      variant="inline"
      interactive
      className={dropTarget ? "cursor-pointer bg-muted" : "cursor-pointer"}
      tabIndex={0}
      onDragOver={(event) => {
        if (!dragging) return
        event.preventDefault()
        onDropTarget(service.instance_id)
      }}
      onDrop={(event) => {
        event.preventDefault()
        const fromId = event.dataTransfer.getData("text/plain")
        if (fromId) onMove(fromId, service.instance_id)
        onDragEnd()
      }}
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
          <button
            type="button"
            draggable
            aria-label={t("dragService", { name: service.service_name })}
            className="shrink-0 cursor-grab text-muted-foreground active:cursor-grabbing"
            onClick={(event) => event.stopPropagation()}
            onDragStart={(event) => {
              event.stopPropagation()
              event.dataTransfer.effectAllowed = "move"
              event.dataTransfer.setData("text/plain", service.instance_id)
              onDragStart(service.instance_id)
            }}
            onDragEnd={onDragEnd}
          >
            <GripVerticalIcon className="size-4" />
          </button>
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
  onMove: (fromId: string, toId: string) => void
  onOpen: (service: ServiceInstance) => void
  onRefresh: () => void
  onRestart: (service: ServiceInstance) => void
}) {
  const [moreService, setMoreService] = useState<ServiceInstance | null>(null)
  const [editService, setEditService] = useState<ServiceInstance | null>(null)
  const [scopeService, setScopeService] = useState<ServiceInstance | null>(null)
  const [dragId, setDragId] = useState<string | null>(null)
  const [overId, setOverId] = useState<string | null>(null)

  const endDrag = () => {
    setDragId(null)
    setOverId(null)
  }

  return (
    <>
      <div>
        {props.services.map((service) => (
          <ServiceRow
            key={service.instance_id}
            busy={props.busy}
            dragging={dragId !== null}
            dropTarget={overId === service.instance_id && dragId !== service.instance_id}
            onAddScope={setScopeService}
            onConnect={props.onConnect}
            onDisconnect={props.onDisconnect}
            onDragEnd={endDrag}
            onDragStart={setDragId}
            onDropTarget={setOverId}
            onEdit={setEditService}
            onMore={setMoreService}
            onMove={props.onMove}
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
