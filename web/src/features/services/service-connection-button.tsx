import { LinkIcon, UnlinkIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { isServiceConnected, isServiceConnecting } from "@/features/services/service-display-status"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ServiceState } from "@/lib/api"

export function isServiceRunning(state?: ServiceState) {
  return isServiceConnected(state)
}

export function isServiceStarting(state?: ServiceState, busy?: string | null, instanceId?: string) {
  return isServiceConnecting(state) || Boolean(instanceId && busy === `connect:${instanceId}`)
}

export function isServiceDisconnecting(busy?: string | null, instanceId?: string) {
  return Boolean(instanceId && busy === `disconnect:${instanceId}`)
}

/**
 * Inner content wrapper that uses CSS grid stacking to size the button
 * to its widest possible state, avoiding layout shift without a magic width value.
 */
function ConnectionButtonContent({
  label,
  sizerLabel,
  spinner = false,
}: {
  label: string
  sizerLabel: string
  spinner?: boolean
}) {
  return (
    <span className="inline-grid [&>*]:col-start-1 [&>*]:row-start-1">
      {/* Invisible sizer: always renders the longest label to reserve width */}
      <span className="invisible" aria-hidden="true">
        {sizerLabel}
      </span>
      {/* Visible content */}
      <span className="flex items-center gap-1.5">
        {spinner ? <Spinner className="size-4" /> : null}
        <span>{label}</span>
      </span>
    </span>
  )
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
  const instanceConnectionBusy = busy === `connect:${instanceId}` || busy === `disconnect:${instanceId}`

  // The longest label across all states — used by the sizer to prevent layout shift
  const sizerLabel = t("disconnecting")

  if (running) {
    return (
      <Button variant={variant} size={size} className={className} onClick={onDisconnect} disabled={disconnecting}>
        <ConnectionButtonContent
          spinner={disconnecting}
          label={disconnecting ? t("disconnecting") : t("disconnect")}
          sizerLabel={sizerLabel}
        />
      </Button>
    )
  }

  if (starting) {
    return (
      <Button variant={variant} size={size} className={className} disabled>
        <ConnectionButtonContent spinner label={t("connecting")} sizerLabel={sizerLabel} />
      </Button>
    )
  }

  return (
    <Button variant={variant} size={size} className={className} onClick={onConnect} disabled={instanceConnectionBusy}>
      <ConnectionButtonContent label={t("connect")} sizerLabel={sizerLabel} />
    </Button>
  )
}

export function ServiceConnectionButtonForEntry({
  busy,
  service,
  onConnect,
  onDisconnect,
}: {
  busy: string | null
  service: ServiceInstance
  onConnect: (service: ServiceInstance) => void
  onDisconnect: (service: ServiceInstance) => void
}) {
  const { t } = useI18n()
  const running = isServiceRunning(service.state)
  const starting = isServiceStarting(service.state, busy, service.instance_id)
  const disconnecting = isServiceDisconnecting(busy, service.instance_id)
  const pending =
    starting ||
    disconnecting ||
    busy === `connect:${service.instance_id}` ||
    busy === `disconnect:${service.instance_id}`
  const label = running
    ? disconnecting
      ? t("disconnecting")
      : t("disconnect")
    : starting
      ? t("connecting")
      : t("connect")

  return (
    <Button
      variant="outline"
      size="icon-sm"
      aria-label={label}
      title={label}
      disabled={pending}
      onClick={() => (running ? onDisconnect(service) : onConnect(service))}
    >
      {pending ? <Spinner /> : running ? <UnlinkIcon /> : <LinkIcon />}
    </Button>
  )
}
