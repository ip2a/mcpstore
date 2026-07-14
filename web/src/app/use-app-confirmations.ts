import { useState } from "react"

import type { ResetTarget } from "@/features/config/config-view"
import { resetAgentConfig, resetConfig, type ServiceInstance } from "@/lib/api"

type RunAction = (
  label: string,
  action: () => Promise<unknown>,
  onSuccess?: () => Promise<void> | void,
) => Promise<void>

export function useAppConfirmations({
  refreshConfigQueries,
  runAction,
}: {
  refreshConfigQueries: (target: ResetTarget) => Promise<void>
  runAction: RunAction
}) {
  const [deleteTarget, setDeleteTarget] = useState<ServiceInstance | null>(null)
  const [resetTarget, setResetTarget] = useState<ResetTarget | null>(null)

  async function confirmReset(target: ResetTarget) {
    if (target.scope === "store") {
      await runAction("reset:store", resetConfig, () => refreshConfigQueries(target))
    } else {
      await runAction(`reset:${target.agentId}`, () => resetAgentConfig(target.agentId), () => refreshConfigQueries(target))
    }

    setResetTarget(null)
  }

  function closeDeleteDialog(open: boolean) {
    if (!open) setDeleteTarget(null)
  }

  function closeResetDialog(open: boolean) {
    if (!open) setResetTarget(null)
  }

  return {
    closeDeleteDialog,
    closeResetDialog,
    confirmReset,
    deleteTarget,
    resetTarget,
    setDeleteTarget,
    setResetTarget,
  }
}
