import { useQuery } from "@tanstack/react-query"

import { isServiceConnected } from "@/features/services/service-display-status"
import { listInstancePrompts, listInstanceResources, type ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { getServiceTransport } from "@/lib/service-info"

export function ServiceRowMeta({
  service,
  showScope = false,
  toolCount,
}: {
  service: ServiceInstance
  showScope?: boolean
  toolCount?: number
}) {
  const { t } = useI18n()
  const connected = isServiceConnected(service.state)
  const scope =
    service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`
  const transport = getServiceTransport(service)
  const transportLabel = transport !== "unknown" ? transport : "-"

  const canListResources = connected && service.mcp?.capabilities?.resources !== false
  const canListPrompts = connected && service.mcp?.capabilities?.prompts !== false

  const resourcesQuery = useQuery({
    enabled: canListResources,
    queryKey: queryKeys.instanceResources(service),
    queryFn: () => listInstanceResources(service).catch(() => []),
    staleTime: 60_000,
  })
  const promptsQuery = useQuery({
    enabled: canListPrompts,
    queryKey: queryKeys.instancePrompts(service),
    queryFn: () => listInstancePrompts(service).catch(() => []),
    staleTime: 60_000,
  })

  const resourceCount = resourcesQuery.data?.length ?? 0
  const promptCount = promptsQuery.data?.length ?? 0

  const parts = [
    showScope ? scope : null,
    transportLabel,
    connected && toolCount !== undefined && toolCount > 0
      ? t("serviceRowToolCount", { count: toolCount })
      : null,
    resourceCount > 0 ? t("serviceRowResourceCount", { count: resourceCount }) : null,
    promptCount > 0 ? t("serviceRowPromptCount", { count: promptCount }) : null,
  ].filter((part): part is string => Boolean(part))

  return (
    <div className="mt-1 min-w-0 truncate text-sm text-muted-foreground" title={parts.join(" · ")}>
      {parts.join("  ")}
    </div>
  )
}
