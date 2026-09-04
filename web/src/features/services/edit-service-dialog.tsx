import { useState } from "react"

import { EditServiceForm } from "@/features/services/edit-service-form"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
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
        <DialogHeader className="shrink-0 border-b px-4 py-4 @min-[640px]:px-5">
          <DialogTitle>{t("editService")}</DialogTitle>
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
