import { useMcpRegistryEntries } from "@/features/mcp-registry/use-mcp-registry-entries"
import { useInstancePromptsQueries } from "@/features/prompts/queries"
import type { AgentItem, PromptInfo, ServiceInstance } from "@/lib/api"

export function usePromptsRegistry({
  agents,
  services,
}: {
  agents: AgentItem[]
  services: ServiceInstance[]
}) {
  const { reload, visibleEntries, ...registry } = useMcpRegistryEntries<PromptInfo>({
    agents,
    services,
    useEntryQueries: useInstancePromptsQueries,
    matchQuery: matchesPromptQuery,
    noun: "prompts",
  })
  return {
    ...registry,
    visiblePrompts: visibleEntries.map(({ instance, entry }) => ({ instance, prompt: entry })),
    loadPrompts: reload,
  }
}

function matchesPromptQuery(instance: ServiceInstance, prompt: PromptInfo, query: string) {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
  const text =
    `${prompt.name} ${prompt.title || ""} ${prompt.description || ""} ${instance.service_name} ${scopeLabel}`.toLowerCase()
  return text.includes(needle)
}
