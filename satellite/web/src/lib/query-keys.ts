import type { ScopeRef, ScopeView, ServiceAddress } from "@/lib/api"
import { getSelectedHost } from "@/lib/api/backend"

function scopeSeg(scope: ScopeRef): string[] {
  return scope.type === "agent" ? ["agent", scope.agent_id] : ["store"]
}

function viewSeg(view: ScopeView): string[] {
  return view.type === "agent" ? ["agent", view.agent_id] : [view.type]
}

// Resource queries carry the selected host in their key so a stale response
// from one daemon can never populate another daemon's view. Panel queries
// (meta/settings/clients/health) stay host-free.
function hostSeg(): string[] {
  return [getSelectedHost()]
}

export const queryKeys = {
  health: ["health"] as const,
  get scopes() {
    return ["scopes", ...hostSeg()] as const
  },
  scopeServices: (view: ScopeView) =>
    ["scopes", ...hostSeg(), "services", ...viewSeg(view)] as const,
  get instances() {
    return ["instances", ...hostSeg()] as const
  },
  instance: (addr: ServiceAddress) =>
    ["instances", ...hostSeg(), addr.service_name, ...scopeSeg(addr.scope)] as const,
  instanceStatus: (addr: ServiceAddress) => [...queryKeys.instance(addr), "status"] as const,
  instanceAuth: (addr: ServiceAddress) => [...queryKeys.instance(addr), "auth"] as const,
  instanceTools: (addr: ServiceAddress) => [...queryKeys.instance(addr), "tools"] as const,
  instanceResources: (addr: ServiceAddress) => [...queryKeys.instance(addr), "resources"] as const,
  instanceResourceTemplates: (addr: ServiceAddress) =>
    [...queryKeys.instance(addr), "resource-templates"] as const,
  instancePrompts: (addr: ServiceAddress) => [...queryKeys.instance(addr), "prompts"] as const,
  get agents() {
    return ["agents", ...hostSeg()] as const
  },
  agent: (agentId: string) => ["agents", ...hostSeg(), agentId] as const,
  agentServices: (agentId: string) => ["agents", ...hostSeg(), agentId, "instances"] as const,
  get config() {
    return ["config", ...hostSeg(), "store"] as const
  },
  agentConfig: (agentId: string) => ["config", ...hostSeg(), "agent", agentId] as const,
  get cacheHealth() {
    return ["cache", ...hostSeg(), "health"] as const
  },
  get cacheInspect() {
    return ["cache", ...hostSeg(), "inspect"] as const
  },
  meta: ["meta"] as const,
  settings: ["settings"] as const,
  overrides: (kind: string) => ["overrides", ...hostSeg(), kind] as const,
  serviceOverride: (serviceName: string, scope?: ScopeRef) =>
    [
      ...queryKeys.instance({ service_name: serviceName, scope: scope ?? { type: "store" } }),
      "service-override",
    ] as const,
  clients: ["clients"] as const,
  clientVersion: (clientId: string) => ["clients", clientId, "version"] as const,
  clientPlugins: ["clients", "plugins"] as const,
  clientPluginMarket: ["clients", "plugins", "market"] as const,
  servicesMarketFeatured: ["clients", "plugins", "market", "services"] as const,
  servicesMarketRegistry: (q: string) =>
    ["clients", "plugins", "market", "services", "registry", q] as const,
  wiredClients: ["clients", "wired"] as const,
  get gatewayStatus() {
    return ["gateway", ...hostSeg(), "status"] as const
  },
  get gatewayProjection() {
    return ["gateway", ...hostSeg(), "projection"] as const
  },
  get usageStats() {
    return ["gateway", ...hostSeg(), "usage-stats"] as const
  },
  get dailyUsageStats() {
    return ["gateway", ...hostSeg(), "daily-usage-stats"] as const
  },
  get heatmapStats() {
    return ["gateway", ...hostSeg(), "heatmap-stats"] as const
  },
  frameActivityStats: (hours: number, bucketMs?: number) =>
    ["gateway", ...hostSeg(), "frame-activity-stats", hours, bucketMs ?? 0] as const,
  frameActivityByService: (hours: number, bucketMs?: number) =>
    ["gateway", ...hostSeg(), "frame-activity-by-service", hours, bucketMs ?? 0] as const,
  frameActivityByMethod: (hours: number, bucketMs?: number) =>
    ["gateway", ...hostSeg(), "frame-activity-by-method", hours, bucketMs ?? 0] as const,
  get gatewayHistory() {
    return ["gateway", ...hostSeg(), "requests-history"] as const
  },
  gatewayHistoryPage: (limit: number, offset: number) =>
    ["gateway", ...hostSeg(), "requests-history", limit, offset] as const,
  get gatewayFrameSessions() {
    return ["gateway", ...hostSeg(), "frames-sessions"] as const
  },
  gatewayFrameSessionsPage: (face: string, limit: number, offset: number) =>
    ["gateway", ...hostSeg(), "frames-sessions", face, limit, offset] as const,
  gatewayFrameHistory: (sessionId: string) =>
    ["gateway", ...hostSeg(), "frames-history", sessionId] as const,
  gatewayFramesRecent: (face: string, limit: number) =>
    ["gateway", ...hostSeg(), "frames-recent", face, limit] as const,
}
