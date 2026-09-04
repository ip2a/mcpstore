import { AlertCircleIcon, LinkIcon, Loader2Icon, UnlinkIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { deriveServiceDisplayStatus, type ServiceDisplayStatus } from "@/features/services/service-display-status"
import type { ServiceState } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const STATUS_LABELS = {
  connected: "statusConnected",
  connecting: "statusConnecting",
  disconnected: "statusDisconnected",
  error: "statusError",
} as const

const STATUS_ICONS = {
  connected: LinkIcon,
  connecting: Loader2Icon,
  disconnected: UnlinkIcon,
  error: AlertCircleIcon,
} as const

function statusVariant(status: ServiceDisplayStatus) {
  if (status === "connected") return "default" as const
  if (status === "error") return "destructive" as const
  return "secondary" as const
}

export function ServiceStatusBadge({ state }: { state: ServiceState }) {
  const { t } = useI18n()
  const status = deriveServiceDisplayStatus(state)

  return (
    <Badge variant={statusVariant(status)} title={`${state.readiness.reason} · ${state.health}`}>
      {t(STATUS_LABELS[status])}
    </Badge>
  )
}

/** Compact icon mark for list rows — placed after the service name. */
export function ServiceConnectionMark({ state, className }: { state: ServiceState; className?: string }) {
  const { t } = useI18n()
  const status = deriveServiceDisplayStatus(state)
  const Icon = STATUS_ICONS[status]
  const label = t(STATUS_LABELS[status])
  const title = `${label} · ${state.readiness.reason} · ${state.health}`

  return (
    <Badge
      variant={statusVariant(status)}
      className={cn("size-4 shrink-0 gap-0 px-0 [&>svg]:size-2.5", className)}
      title={title}
      aria-label={label}
    >
      <Icon className={cn(status === "connecting" && "animate-spin")} />
    </Badge>
  )
}
