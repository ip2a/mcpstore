import type { AgentItem } from "@/lib/api"

export function getAgentId(agent: AgentItem) {
  return String(agent.agent_id || agent.id || "")
}

export function getAgentServices(agent: AgentItem) {
  return (agent.services || agent.service_names || []).map(String)
}
