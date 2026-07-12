import { useState } from "react"

import { EditServiceForm } from "@/features/services/edit-service-form"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceEntry } from "@/lib/api"

export function EditServiceDialog(props: {
  onOpenChange: (open: boolean) => void
  onUpdated: () => Promise<void>
  open: boolean
  service: ServiceEntry | null
}) {
  const { t } = useI18n()
  const [session, setSession] = useState(0)

  function onOpenChange(next: boolean) {
    props.onOpenChange(next)
    if (next && props.service) setSession((value) => value + 1)
  }

  return (
    <Dialog open={props.open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(90vh,48rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="border-b px-6 py-4">
          <DialogTitle>{t("editService")}</DialogTitle>
          <DialogDescription>{props.service ? t("editServiceDescription", { name: props.service.name }) : null}</DialogDescription>
        </DialogHeader>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
          {props.service ? (
            <EditServiceForm
              key={`${props.service.name}:${session}`}
              service={props.service}
              onUpdated={props.onUpdated}
              onCancel={() => props.onOpenChange(false)}
            />
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  )
}
