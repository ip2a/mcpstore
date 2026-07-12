import { RefreshCwIcon } from "lucide-react"

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
import type { ResetTarget } from "@/features/config/config-view"

export function ResetConfigDialog({ target, onOpenChange, onConfirm }: { target: ResetTarget | null; onOpenChange: (open: boolean) => void; onConfirm: (target: ResetTarget) => void }) {
  const { t } = useI18n()
  const label = target?.scope === "agent" ? `${t("agent")} ${target.agentId}` : t("store")

  return (
    <AlertDialog open={Boolean(target)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <RefreshCwIcon />
          </AlertDialogMedia>
          <AlertDialogTitle>{t("resetConfigTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{target ? t("resetConfigDescription", { label }) : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("cancel")}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => target && onConfirm(target)}>{t("reset")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
