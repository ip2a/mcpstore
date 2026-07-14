import { Badge } from "@/components/ui/badge"
import type { ConnectionStatus } from "@/lib/api"

export function ServiceStatusBadge({ status }: { status?: ConnectionStatus }) {
  const label = status || "unknown"
  return <Badge variant={label === "connected" ? "default" : label === "error" ? "destructive" : "secondary"}>{label}</Badge>
}
