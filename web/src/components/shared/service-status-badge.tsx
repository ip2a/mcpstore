import { Badge } from "@/components/ui/badge"
import type { ReadinessStatus } from "@/lib/api"

export function ServiceStatusBadge({ status }: { status: ReadinessStatus }) {
  return <Badge variant={status === "ready" ? "default" : status === "not_ready" ? "destructive" : "secondary"}>{status}</Badge>
}
