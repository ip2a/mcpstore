import { LinkIcon, UnlinkIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useI18n } from "@/lib/i18n-context"
import type { ConnectionStatus, ServiceInstance } from "@/lib/api"

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
  instanceId,
  status,
  onConnect,
  onDisconnect,
  size = "sm",
  variant = "outline",
}: {
  busy: string | null
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

  if (connected) {
    return (
      <Button variant={variant} size={size} onClick={onDisconnect} disabled={Boolean(busy)}>
        {disconnecting ? <Spinner data-icon="inline-start" /> : <UnlinkIcon data-icon="inline-start" />}
        {disconnecting ? t("disconnecting") : t("disconnect")}
      </Button>
    )
  }

  if (connecting) {
    return (
      <Button variant={variant} size={size} disabled>
        <Spinner data-icon="inline-start" />
        {t("connecting")}
      </Button>
    )
  }

  return (
    <Button variant={variant} size={size} onClick={onConnect} disabled={Boolean(busy)}>
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
