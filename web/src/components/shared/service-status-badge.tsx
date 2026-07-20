import { Badge } from "@/components/ui/badge"
import { deriveServiceDisplayStatus } from "@/features/services/service-display-status"
import type { ServiceState } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

const STATUS_LABELS = {
  connected: "statusConnected",
  connecting: "statusConnecting",
  disconnected: "statusDisconnected",
  error: "statusError",
} as const

export function ServiceStatusBadge({ state }: { state: ServiceState }) {
  const { t } = useI18n()
  const status = deriveServiceDisplayStatus(state)
  const variant = status === "connected" ? "default" : status === "error" ? "destructive" : "secondary"

  return (
    <Badge variant={variant} title={`${state.readiness.reason} · ${state.health}`}>
      {t(STATUS_LABELS[status])}
    </Badge>
  )
}
