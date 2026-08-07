import type { ScopeRef, ScopeView, ServiceAddress } from "@/lib/api"

function scopeSeg(scope: ScopeRef): string[] {
  return scope.type === "agent" ? ["agent", scope.agent_id] : ["store"]
}

function viewSeg(view: ScopeView): string[] {
  return view.type === "agent" ? ["agent", view.agent_id] : [view.type]
}

export const queryKeys = {
  health: ["health"] as const,
  scopes: ["scopes"] as const,
  scopeServices: (view: ScopeView) =>
    ["scopes", "services", ...viewSeg(view)] as const,
  instances: ["instances"] as const,
  instance: (addr: ServiceAddress) =>
    ["instances", addr.service_name, ...scopeSeg(addr.scope)] as const,
  instanceStatus: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "status"] as const,
  instanceAuth: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "auth"] as const,
  instanceTools: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "tools"] as const,
  instanceResources: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "resources"] as const,
  instanceResourceTemplates: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "resource-templates"] as const,
  instancePrompts: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "prompts"] as const,
  agents: ["agents"] as const,
  agent: (agentId: string) => ["agents", agentId] as const,
  agentServices: (agentId: string) => ["agents", agentId, "instances"] as const,
  config: ["config", "store"] as const,
  agentConfig: (agentId: string) => ["config", "agent", agentId] as const,
  cacheHealth: ["cache", "health"] as const,
  cacheInspect: ["cache", "inspect"] as const,
  meta: ["meta"] as const,
  settings: ["settings"] as const,
}
