import { CoreBadge } from "@/components/layout/core-badge"
import { GatewayBadge } from "@/components/layout/gateway-badge"

export function AppBottomBar() {
  return (
    <footer className="flex h-8 shrink-0 items-center justify-end gap-1 border-t bg-card/40 px-2">
      <GatewayBadge />
      <CoreBadge />
    </footer>
  )
}
