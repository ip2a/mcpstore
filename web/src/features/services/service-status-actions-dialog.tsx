import { RefreshCwIcon, Trash2Icon } from "lucide-react"

import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import {
  isServiceConnected,
  ServiceConnectionButton,
} from "@/features/services/service-connection-button"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceEntry } from "@/lib/api"

export function ServiceStatusActionsDialog({
  busy,
  open,
  service,
  serviceStatus,
  onConnect,
  onDelete,
  onDisconnect,
  onOpenChange,
  onRestart,
}: {
  busy: string | null
  open: boolean
  service: ServiceEntry
  serviceStatus?: string
  onConnect: () => void
  onDelete: () => void
  onDisconnect: () => void
  onOpenChange: (open: boolean) => void
  onRestart: () => void
}) {
  const { t } = useI18n()
  const connected = isServiceConnected(serviceStatus)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("serviceStatus")}</DialogTitle>
          <DialogDescription className="font-mono">{service.name}</DialogDescription>
        </DialogHeader>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t("current")}</span>
          <ServiceStatusBadge status={serviceStatus} />
        </div>
        <div className="flex flex-col gap-2">
          <ServiceConnectionButton
            busy={busy}
            serviceName={service.name}
            status={serviceStatus}
            onConnect={onConnect}
            onDisconnect={onDisconnect}
            size="default"
            variant={connected ? "outline" : "default"}
          />
          <Button variant="outline" onClick={onRestart} disabled={Boolean(busy)}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("restart")}
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              onOpenChange(false)
              onDelete()
            }}
            disabled={Boolean(busy)}
          >
            <Trash2Icon data-icon="inline-start" />
            {t("delete")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
