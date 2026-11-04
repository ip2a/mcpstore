import type { ResetTarget } from "@/features/config/config-view"
import { ResetConfigDialog } from "@/features/config/reset-config-dialog"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { DeleteServiceDialog } from "@/features/services/delete-service-dialog"
import { RunToolDialog, ToolDetailDialog, ToolResultDialog, type ToolDetailState, type ToolDialogState, type ToolResultState } from "@/features/tools/tool-dialogs"
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
  onToolDialogOpenChange,
  onToolResultOpenChange,
  resetTarget,
  toolDetail,
  toolDialog,
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
  onToolDialogOpenChange: (open: boolean) => void
  onToolResultOpenChange: (open: boolean) => void
  resetTarget: ResetTarget | null
  toolDetail: ToolDetailState
  toolDialog: ToolDialogState
  toolResult: ToolResultState
}) {
  return (
    <>
      <RunToolDialog state={toolDialog} onOpenChange={onToolDialogOpenChange} />
      <ToolResultDialog state={toolResult} onOpenChange={onToolResultOpenChange} />
      <ToolDetailDialog
        state={toolDetail}
        onOpenChange={onToolDetailOpenChange}
        onRun={onRunToolFromDetail}
      />
      <SwitchCacheDialog open={cacheDialogOpen} current={backend} onOpenChange={onCacheDialogOpenChange} onChanged={onCacheChanged} />
      <DeleteServiceDialog service={deleteTarget} onOpenChange={onDeleteDialogOpenChange} onConfirm={onConfirmDelete} />
      <ResetConfigDialog target={resetTarget} onOpenChange={onResetDialogOpenChange} onConfirm={onConfirmReset} />
    </>
  )
}
