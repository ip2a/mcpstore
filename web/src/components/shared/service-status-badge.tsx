import { Badge } from "@/components/ui/badge"
import type { ConnectionStatus } from "@/lib/api"

export function ServiceStatusBadge({ status }: { status?: ConnectionStatus }) {
  const label = status || "Unknown"
  return <Badge variant={label === "Connected" ? "default" : label === "Error" ? "destructive" : "secondary"}>{label}</Badge>
}
