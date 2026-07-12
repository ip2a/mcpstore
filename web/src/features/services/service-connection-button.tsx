import { LinkIcon, UnlinkIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useI18n } from "@/lib/i18n-context"
import type { ConnectionStatus, ServiceEntry } from "@/lib/api"

export function isServiceConnected(status?: ConnectionStatus) {
  return status?.toLowerCase() === "connected"
}

export function isServiceConnecting(status?: ConnectionStatus, busy?: string | null, serviceName?: string) {
  return status?.toLowerCase() === "connecting" || Boolean(serviceName && busy === `connect:${serviceName}`)
}

export function isServiceDisconnecting(busy?: string | null, serviceName?: string) {
  return Boolean(serviceName && busy === `disconnect:${serviceName}`)
}

export function ServiceConnectionButton({
  busy,
  serviceName,
  status,
  onConnect,
  onDisconnect,
  size = "sm",
  variant = "outline",
}: {
  busy: string | null
  serviceName: string
  status?: ConnectionStatus
  onConnect: () => void
  onDisconnect: () => void
  size?: "default" | "sm" | "lg" | "icon"
  variant?: "default" | "outline" | "destructive" | "secondary" | "ghost" | "link"
}) {
  const { t } = useI18n()
  const connected = isServiceConnected(status)
  const connecting = isServiceConnecting(status, busy, serviceName)
  const disconnecting = isServiceDisconnecting(busy, serviceName)

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
  service: ServiceEntry
  onConnect: (service: ServiceEntry) => void
  onDisconnect: (service: ServiceEntry) => void
  size?: "default" | "sm" | "lg" | "icon"
  variant?: "default" | "outline" | "destructive" | "secondary" | "ghost" | "link"
}) {
  return (
    <ServiceConnectionButton
      busy={busy}
      serviceName={service.name}
      status={service.status}
      onConnect={() => onConnect(service)}
      onDisconnect={() => onDisconnect(service)}
      size={size}
      variant={variant}
    />
  )
}
