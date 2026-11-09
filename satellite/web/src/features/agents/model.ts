import type { AgentItem } from "@/lib/api"

export function getAgentId(agent: AgentItem) {
  return agent.agent_id
}
