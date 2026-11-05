import { ServiceTransitionIcon } from "@/components/shared/service-transition-icon"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { isServiceConnected, isServiceConnecting } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceState } from "@/lib/api"

export function isServiceRunning(state?: ServiceState) {
  return isServiceConnected(state)
}

export function isServiceStarting(state?: ServiceState, busy?: string | null, instanceId?: string) {
  return isServiceConnecting(state) || Boolean(instanceId && busy === `connect:${instanceId}`)
}

export function isServiceDisconnecting(busy?: string | null, instanceId?: string) {
  return Boolean(instanceId && busy === `disconnect:${instanceId}`)
}

export function ServiceConnectionButton({
  busy,
  className,
  instanceId,
  state,
  onConnect,
  onDisconnect,
  size = "sm",
  variant = "outline",
}: {
  busy: string | null
  className?: string
  instanceId: string
  state?: ServiceState
  onConnect: () => void
  onDisconnect: () => void
  size?: "default" | "sm" | "lg" | "icon"
  variant?: "default" | "outline" | "destructive" | "secondary" | "ghost" | "link"
}) {
  const { t } = useI18n()
  const running = isServiceRunning(state)
  const starting = isServiceStarting(state, busy, instanceId)
  const disconnecting = isServiceDisconnecting(busy, instanceId)
  const instanceConnectionBusy =
    busy === `connect:${instanceId}` || busy === `disconnect:${instanceId}`

  if (running) {
    return (
      <Button
        variant={variant}
        size={size}
        className={className}
        onClick={onDisconnect}
        disabled={disconnecting}
      >
        {disconnecting ? (
          <Spinner data-icon="inline-start" />
        ) : (
          <ServiceTransitionIcon direction="disconnect" data-icon="inline-start" />
        )}
        {disconnecting ? t("disconnecting") : t("disconnect")}
      </Button>
    )
  }

  if (starting) {
    return (
      <Button variant={variant} size={size} className={className} disabled>
        <Spinner data-icon="inline-start" />
        {t("connecting")}
      </Button>
    )
  }

  return (
    <Button
      variant={variant}
      size={size}
      className={className}
      onClick={onConnect}
      disabled={instanceConnectionBusy}
    >
      <ServiceTransitionIcon direction="connect" data-icon="inline-start" />
      {t("connect")}
    </Button>
  )
}
