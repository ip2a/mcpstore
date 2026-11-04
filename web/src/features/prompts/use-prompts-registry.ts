import { useMemo } from "react"
import { toast } from "sonner"

import { useMcpRegistryScope } from "@/features/mcp-registry/use-mcp-registry-scope"
import { useInstancePromptsQueries } from "@/features/prompts/queries"
import type { AgentItem, PromptInfo, ServiceInstance } from "@/lib/api"

export function usePromptsRegistry({ agents, services }: { agents: AgentItem[]; services: ServiceInstance[] }) {
  const scopeState = useMcpRegistryScope({ agents, services })
  const promptQueries = useInstancePromptsQueries(scopeState.selectedInstances)
  const prompts = useMemo(
    () =>
      promptQueries.flatMap((result, index) =>
        (result.data || []).map((prompt) => ({
          instance: scopeState.selectedInstances[index],
          prompt,
        })),
      ),
    [promptQueries, scopeState.selectedInstances],
  )
  const queryError = scopeState.agentServicesQuery.error || promptQueries.find((result) => result.error)?.error
  const errorMessage =
    queryError instanceof Error ? queryError.message : queryError ? String(queryError) : "Failed to load prompts"
  const loading = scopeState.agentServicesQuery.isFetching || promptQueries.some((result) => result.isFetching)

  async function loadPrompts() {
    try {
      if (scopeState.scope === "agent") {
        const result = await scopeState.agentServicesQuery.refetch()
        if (result.error) throw result.error
      }
      const results = await Promise.all(promptQueries.map((result) => result.refetch()))
      const failed = results.find((result) => result.error)
      if (failed?.error) throw failed.error
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to load prompts")
    }
  }

  const visiblePrompts = prompts.filter(({ instance, prompt }) => matchesPromptQuery(instance, prompt, scopeState.query))

  return {
    ...scopeState,
    error: queryError,
    errorMessage,
    loadPrompts,
    loading,
    visiblePrompts,
  }
}

function matchesPromptQuery(instance: ServiceInstance, prompt: PromptInfo, query: string) {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id
  const text = `${prompt.name} ${prompt.title || ""} ${prompt.description || ""} ${instance.service_name} ${scopeLabel}`.toLowerCase()
  return text.includes(needle)
}
