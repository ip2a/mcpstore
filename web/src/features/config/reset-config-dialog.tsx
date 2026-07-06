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
import type { ResetTarget } from "@/features/config/config-view"

export function ResetConfigDialog({ target, onOpenChange, onConfirm }: { target: ResetTarget | null; onOpenChange: (open: boolean) => void; onConfirm: (target: ResetTarget) => void }) {
  const label = target?.scope === "agent" ? `Agent ${target.agentId}` : "Store"
  return (
    <AlertDialog open={Boolean(target)} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="text-destructive">
            <RefreshCwIcon />
          </AlertDialogMedia>
          <AlertDialogTitle>Reset config?</AlertDialogTitle>
          <AlertDialogDescription>{target ? `${label} config will be reset.` : null}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => target && onConfirm(target)}>Reset</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
