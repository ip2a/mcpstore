import { useState } from "react"

import { EditServiceForm } from "@/features/services/edit-service-form"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

export function EditServiceDialog(props: {
  onOpenChange: (open: boolean) => void
  onUpdated: () => Promise<void>
  open: boolean
  service: ServiceInstance | null
}) {
  const { t } = useI18n()
  const [session, setSession] = useState(0)
  const serviceLabel = props.service
    ? `${props.service.service_name} · ${props.service.scope.type === "store" ? t("store") : `${t("agent")} ${props.service.scope.agent_id}`}`
    : null

  function onOpenChange(next: boolean) {
    props.onOpenChange(next)
    if (next && props.service) setSession((value) => value + 1)
  }

  return (
    <Dialog open={props.open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[78dvh] w-[min(84vw,68rem)]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-3 @min-[640px]:px-5 @min-[640px]:py-3.5">
          <DialogTitle>{t("editService")}</DialogTitle>
          <DialogDescription>{serviceLabel ? t("editServiceDescription", { name: serviceLabel }) : null}</DialogDescription>
        </DialogHeader>
        {props.service ? (
          <EditServiceForm
            key={`${props.service.instance_id}:${session}`}
            service={props.service}
            onUpdated={props.onUpdated}
            onCancel={() => props.onOpenChange(false)}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  )
}
