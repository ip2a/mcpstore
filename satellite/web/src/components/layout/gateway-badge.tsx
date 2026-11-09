import { useNavigate } from "react-router-dom"

import { useGatewayStatusQuery } from "@/features/clients/queries"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export function GatewayBadge() {
  const { t } = useI18n()
  const navigate = useNavigate()
  const status = useGatewayStatusQuery()

  const label = t("gateway")
  const running = status.data?.running === true
  const title = status.isPending
    ? label
    : running
      ? `${label} · ${t("homeRunning")}`
      : `${label} · ${t("gatewayStopped")}`

  const dot = status.isPending
    ? "bg-muted-foreground/40"
    : running
      ? "bg-emerald-500"
      : "bg-destructive"

  return (
    <button
      type="button"
      aria-label={title}
      title={title}
      onClick={() => navigate("/gateway")}
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
