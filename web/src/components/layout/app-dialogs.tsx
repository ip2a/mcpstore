import type { ResetTarget } from "@/features/config/config-view"
import { ResetConfigDialog } from "@/features/config/reset-config-dialog"
import { SwitchCacheDialog } from "@/features/cache/switch-cache-dialog"
import { DeleteServiceDialog } from "@/features/services/delete-service-dialog"
import { SettingsDialog } from "@/features/settings/settings-dialog"
import { RunToolDialog, ToolDetailDialog, type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
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
  onSettingsDialogOpenChange,
  onToolDetailOpenChange,
  onToolDialogOpenChange,
  resetTarget,
  settingsDialogOpen,
  toolDetail,
  toolDialog,
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
  onSettingsDialogOpenChange: (open: boolean) => void
  onToolDetailOpenChange: (open: boolean) => void
  onToolDialogOpenChange: (open: boolean) => void
  resetTarget: ResetTarget | null
  settingsDialogOpen: boolean
  toolDetail: ToolDetailState
  toolDialog: ToolDialogState
}) {
  return (
    <>
      <RunToolDialog state={toolDialog} onOpenChange={onToolDialogOpenChange} />
      <ToolDetailDialog
        state={toolDetail}
        onOpenChange={onToolDetailOpenChange}
        onRun={onRunToolFromDetail}
      />
      <SwitchCacheDialog open={cacheDialogOpen} current={backend} onOpenChange={onCacheDialogOpenChange} onChanged={onCacheChanged} />
      <SettingsDialog open={settingsDialogOpen} onOpenChange={onSettingsDialogOpenChange} />
      <DeleteServiceDialog service={deleteTarget} onOpenChange={onDeleteDialogOpenChange} onConfirm={onConfirmDelete} />
      <ResetConfigDialog target={resetTarget} onOpenChange={onResetDialogOpenChange} onConfirm={onConfirmReset} />
    </>
  )
}
