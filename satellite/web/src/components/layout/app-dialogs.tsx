import type { ResetTarget } from "@/features/config/reset-config-dialog"
import { ResetConfigDialog } from "@/features/config/reset-config-dialog"
import { SwitchKvDialog } from "@/features/kv/switch-kv-dialog"
import { DeleteServiceDialog } from "@/features/services/delete-service-dialog"
import {
  ToolDetailDialog,
  ToolResultDialog,
  type ToolDetailState,
  type ToolResultState,
} from "@/features/tools/tool-dialogs"
import type { KvStoreName, ServiceInstance } from "@/lib/api"

export function AppDialogs({
  kvStore,
  kvDialogOpen,
  deleteTarget,
  onKvChanged,
  onKvDialogOpenChange,
  onConfirmDelete,
  onConfirmReset,
  onDeleteDialogOpenChange,
  onResetDialogOpenChange,
  onRunToolFromDetail,
  onToolDetailOpenChange,
  onToolResultOpenChange,
  resetTarget,
  toolDetail,
  toolResult,
}: {
  kvStore?: KvStoreName
  kvDialogOpen: boolean
  deleteTarget: ServiceInstance | null
  onKvChanged: () => Promise<void>
  onKvDialogOpenChange: (open: boolean) => void
  onConfirmDelete: (service: ServiceInstance) => void
  onConfirmReset: (target: ResetTarget) => void
  onDeleteDialogOpenChange: (open: boolean) => void
  onResetDialogOpenChange: (open: boolean) => void
  onRunToolFromDetail: (state: NonNullable<ToolDetailState>) => void
  onToolDetailOpenChange: (open: boolean) => void
  onToolResultOpenChange: (open: boolean) => void
  resetTarget: ResetTarget | null
  toolDetail: ToolDetailState
  toolResult: ToolResultState
}) {
  return (
    <>
      <ToolResultDialog state={toolResult} onOpenChange={onToolResultOpenChange} />
      <ToolDetailDialog
        state={toolDetail}
        onOpenChange={onToolDetailOpenChange}
        onRun={onRunToolFromDetail}
      />
      <SwitchKvDialog
        open={kvDialogOpen}
        current={kvStore}
        onOpenChange={onKvDialogOpenChange}
        onChanged={onKvChanged}
      />
      <DeleteServiceDialog
        service={deleteTarget}
        onOpenChange={onDeleteDialogOpenChange}
        onConfirm={onConfirmDelete}
      />
      <ResetConfigDialog
        target={resetTarget}
        onOpenChange={onResetDialogOpenChange}
        onConfirm={onConfirmReset}
      />
    </>
  )
}
