import { useQuery } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { getMeta } from "@/lib/api"
import { getCoreUrlStatus, onCoreUrlChanged, type CoreUrlStatus } from "@/lib/api/backend"
import { parseHostsPayload } from "@/lib/api/hosts"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"
import { useUiStore } from "@/stores/ui-store"

/**
 * Persistent indicator of which core daemon requests target and how the
 * active host's URL list is holding up: emerald = preferred URL, amber =
 * serving from a failover URL, red = all candidates unreachable.
 */
export function CoreBadge() {
  const setSettingsDialogOpen = useUiStore((state) => state.setSettingsDialogOpen)
  const meta = useQuery({ queryKey: queryKeys.meta, queryFn: getMeta })
  const [status, setStatus] = useState<CoreUrlStatus>(getCoreUrlStatus)

  useEffect(() => onCoreUrlChanged(() => setStatus(getCoreUrlStatus())), [])

  let label = ""
  const hosts = meta.data?.settings?.hosts
  if (hosts) {
    try {
      label = parseHostsPayload(hosts).active
    } catch {
      label = ""
    }
  }

  const dot = status.unreachable
    ? "bg-destructive"
    : status.primary
      ? "bg-emerald-500"
      : "bg-amber-500"

  return (
    <button
      type="button"
      aria-label={label}
      title={`${label ? `${label} · ` : ""}${status.url}`}
      onClick={() => setSettingsDialogOpen(true)}
      className={cn(
        "flex min-w-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs text-muted-foreground",
        "transition-colors hover:bg-muted/60 hover:text-foreground",
      )}
    >
      <span aria-hidden="true" className={cn("size-2 shrink-0 rounded-full", dot)} />
      <span className="truncate font-mono">{label}</span>
    </button>
  )
}
