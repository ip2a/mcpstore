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
import type { ServiceEntry } from "@/lib/api"

export function DeleteServiceDialog({ service, onOpenChange, onConfirm }: { service: ServiceEntry | null; onOpenChange: (open: boolean) => void; onConfirm: (service: ServiceEntry) => void }) {
  const { t } = useI18n()

  return (
    <AlertDialog open={Boolean(service)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <Trash2Icon />
          </AlertDialogMedia>
          <AlertDialogTitle>{t("deleteServiceTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{service ? t("deleteServiceDescription", { name: service.name }) : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("cancel")}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => service && onConfirm(service)}>{t("delete")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
