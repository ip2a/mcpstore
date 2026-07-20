import { LinkIcon, UnlinkIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { isServiceConnected, isServiceConnecting } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ServiceState } from "@/lib/api"
import { cn } from "@/lib/utils"

const CONNECTION_BUTTON_WIDTH_CLASS = "w-[7.75rem] justify-center"

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
  const buttonClassName = cn(size === "sm" && CONNECTION_BUTTON_WIDTH_CLASS, className)

  if (running) {
    return (
      <Button variant={variant} size={size} className={buttonClassName} onClick={onDisconnect} disabled={Boolean(busy)}>
        {disconnecting ? <Spinner data-icon="inline-start" /> : <UnlinkIcon data-icon="inline-start" />}
        {disconnecting ? t("disconnecting") : t("disconnect")}
      </Button>
    )
  }

  if (starting) {
    return (
      <Button variant={variant} size={size} className={buttonClassName} disabled>
        <Spinner data-icon="inline-start" />
        {t("connecting")}
      </Button>
    )
  }

  return (
    <Button variant={variant} size={size} className={buttonClassName} onClick={onConnect} disabled={Boolean(busy)}>
      <LinkIcon data-icon="inline-start" />
      {t("connect")}
    </Button>
  )
}

export function ServiceConnectionButtonForEntry({
  busy,
  service,
  onConnect,
  onDisconnect,
  size = "sm",
  variant = "outline",
}: {
  busy: string | null
  service: ServiceInstance
  onConnect: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
  size?: "default" | "sm" | "lg" | "icon"
  variant?: "default" | "outline" | "destructive" | "secondary" | "ghost" | "link"
}) {
  return (
    <ServiceConnectionButton
      busy={busy}
      instanceId={service.instance_id}
      state={service.state}
      onConnect={() => onConnect(service)}
      onDisconnect={() => onDisconnect(service)}
      size={size}
      variant={variant}
    />
  )
}
