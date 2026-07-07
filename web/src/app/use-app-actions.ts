import { useState } from "react"
import { toast } from "sonner"

type AppAction = () => Promise<unknown>
type AppActionSuccess = () => Promise<void> | void

export function useAppActions(refresh: () => Promise<void>) {
  const [busy, setBusy] = useState<string | null>(null)

  async function runAction(label: string, action: AppAction, onSuccess?: AppActionSuccess) {
    setBusy(label)
    try {
      await action()
      toast.success("操作已完成")
      await refresh()
      await onSuccess?.()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败")
    } finally {
      setBusy(null)
    }
  }

  return { busy, runAction }
}
