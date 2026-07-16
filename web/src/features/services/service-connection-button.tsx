import { LinkIcon, UnlinkIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useI18n } from "@/lib/i18n-context"
import type { ConnectionStatus, ServiceInstance } from "@/lib/api"
import { cn } from "@/lib/utils"

const CONNECTION_BUTTON_WIDTH_CLASS = "w-[7.75rem] justify-center"

export function isServiceConnected(status?: ConnectionStatus) {
  return status?.toLowerCase() === "connected"
}

export function isServiceConnecting(status?: ConnectionStatus, busy?: string | null, instanceId?: string) {
  return status === "connecting" || Boolean(instanceId && busy === `connect:${instanceId}`)
}

export function isServiceDisconnecting(busy?: string | null, instanceId?: string) {
  return Boolean(instanceId && busy === `disconnect:${instanceId}`)
}

export function ServiceConnectionButton({
  busy,
  className,
  instanceId,
  status,
  onConnect,
  onDisconnect,
  size = "sm",
  variant = "outline",
}: {
  busy: string | null
  className?: string
  instanceId: string
  status?: ConnectionStatus
  onConnect: () => void
  onDisconnect: () => void
  size?: "default" | "sm" | "lg" | "icon"
  variant?: "default" | "outline" | "destructive" | "secondary" | "ghost" | "link"
}) {
  const { t } = useI18n()
  const connected = isServiceConnected(status)
  const connecting = isServiceConnecting(status, busy, instanceId)
  const disconnecting = isServiceDisconnecting(busy, instanceId)
  const buttonClassName = cn(size === "sm" && CONNECTION_BUTTON_WIDTH_CLASS, className)

  if (connected) {
    return (
      <Button variant={variant} size={size} className={buttonClassName} onClick={onDisconnect} disabled={Boolean(busy)}>
        {disconnecting ? <Spinner data-icon="inline-start" /> : <UnlinkIcon data-icon="inline-start" />}
        {disconnecting ? t("disconnecting") : t("disconnect")}
      </Button>
    )
  }

  if (connecting) {
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
      status={service.status}
      onConnect={() => onConnect(service)}
      onDisconnect={() => onDisconnect(service)}
      size={size}
      variant={variant}
    />
  )
}
