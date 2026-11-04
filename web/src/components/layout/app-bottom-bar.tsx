import { CoreBadge } from "@/components/layout/core-badge"
import { GatewayBadge } from "@/components/layout/gateway-badge"
import { MarkIcon } from "@/components/shared/mark-icon"

export function AppBottomBar() {
  return (
    <footer className="flex h-8 shrink-0 items-center justify-between gap-2 border-t bg-card/40 px-2">
      <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
        <MarkIcon className="size-3.5" />
        <span className="truncate font-mono text-xs">mcpstore</span>
      </span>
      <span className="flex shrink-0 items-center gap-1">
        <GatewayBadge />
        <CoreBadge />
      </span>
    </footer>
  )
}
