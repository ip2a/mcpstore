import type { ResetTarget } from "@/features/config/reset-config-dialog"
import { ResetConfigDialog } from "@/features/config/reset-config-dialog"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { DeleteServiceDialog } from "@/features/services/delete-service-dialog"
import {
  ToolDetailDialog,
  ToolResultDialog,
  type ToolDetailState,
  type ToolResultState,
} from "@/features/tools/tool-dialogs"
import type { CacheBackend, ServiceInstance } from "@/lib/api"

export function AppDialogs({
  backend,
  cacheDialogOpen,
  deleteTarget,
  onCacheChanged,
  onCacheDialogOpenChange,
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
  backend?: CacheBackend
  cacheDialogOpen: boolean
  deleteTarget: ServiceInstance | null
  onCacheChanged: () => Promise<void>
  onCacheDialogOpenChange: (open: boolean) => void
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
      <SwitchCacheDialog
        open={cacheDialogOpen}
        current={backend}
        onOpenChange={onCacheDialogOpenChange}
        onChanged={onCacheChanged}
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
