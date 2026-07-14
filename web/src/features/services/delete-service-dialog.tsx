import { Trash2Icon } from "lucide-react"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"

export function DeleteServiceDialog({ service, onOpenChange, onConfirm }: { service: ServiceInstance | null; onOpenChange: (open: boolean) => void; onConfirm: (service: ServiceInstance) => void }) {
  const { t } = useI18n()
  const serviceLabel = service
    ? `${service.service_name} · ${service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`}`
    : null

  return (
    <AlertDialog open={Boolean(service)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <Trash2Icon />
          </AlertDialogMedia>
          <AlertDialogTitle>{t("deleteServiceTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{serviceLabel ? t("deleteServiceDescription", { name: serviceLabel }) : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("cancel")}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => service && onConfirm(service)}>{t("delete")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
