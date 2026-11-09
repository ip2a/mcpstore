import { useQuery } from "@tanstack/react-query"
import { useEffect, useState } from "react"
import { useNavigate } from "react-router-dom"

import { getHostStatus, getMeta } from "@/lib/api"
import { getSelectedHost, onSelectedHostChanged } from "@/lib/api/backend"
import { parseHostsPayload } from "@/lib/api/hosts"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

export function CoreBadge() {
  const navigate = useNavigate()
  const meta = useQuery({ queryKey: queryKeys.meta, queryFn: getMeta })
  const [selectedHost, setSelectedHost] = useState(getSelectedHost)
  const hostStatus = useQuery({
    queryKey: ["host-status", selectedHost],
    queryFn: () => getHostStatus(selectedHost),
    enabled: selectedHost !== "",
    refetchInterval: 5000,
  })

  useEffect(() => onSelectedHostChanged(() => setSelectedHost(getSelectedHost())), [])

  let label = selectedHost
  const hosts = meta.data?.settings?.hosts
  if (!label && hosts) {
    try {
      label = parseHostsPayload(hosts).active
    } catch {
      label = ""
    }
  }

  const dot = hostStatus.isPending
    ? "bg-muted-foreground/40"
    : hostStatus.data?.online
      ? "bg-emerald-500"
      : "bg-destructive"

  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={() => navigate("/settings")}
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
