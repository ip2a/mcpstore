import { api, appApiUrl, buildQuery, readJson, scopeParams, scopeQuery } from "./client"
import type {
  AgentItem,
  AuthOperationResult,
  AuthStatusView,
  PromptInfo,
  ResourceInfo,
  ResourceTemplateInfo,
  ScopeSummary,
  ScopeView,
  ServiceAddress,
  ServiceInstance,
  ServiceState,
  ToolInfo,
  ToolVisibilityFilter,
} from "../api"

export type { ServiceState }

/** Builds `/services/:name<suffix>` + scope query (+ optional extra params). Doc §17. */
function svcPath(
  addr: ServiceAddress,
  suffix: string,
  extra?: Record<string, string | number | boolean | null | undefined>,
): string {
  const base = `/services/${encodeURIComponent(addr.service_name)}${suffix}`
  return extra
    ? `${base}${buildQuery({ ...scopeParams(addr.scope), ...extra })}`
    : `${base}${scopeQuery(addr.scope)}`
}

export async function health() {
  // app-face route (the app process's own state {status, store}); the core face has no /health
  return readJson<{ status: string; store: string }>(await fetch(appApiUrl("/health")))
}

export async function listServices(
  view: ScopeView = { type: "store" },
): Promise<ServiceInstance[]> {
  const query =
    view.type === "agent"
      ? buildQuery({ scope: "agent", agent_id: view.agent_id })
      : buildQuery({ scope: view.type })
  const data = await api<{ services: ServiceInstance[] }>(`/services/list${query}`)
  return data.services
}

/** Scope registry (root + store + each agent, each with service_count). Doc §17.3. */
export async function listScopes(): Promise<ScopeSummary[]> {
  const data = await api<{ scopes: ScopeSummary[] }>("/scopes/list")
  return data.scopes
}

export async function listAgents(): Promise<AgentItem[]> {
  const data = await api<{ agents: AgentItem[] }>("/agents/list")
  return data.agents
}

export async function listAgentServices(agentId: string): Promise<ServiceInstance[]> {
  const data = await api<{ services: ServiceInstance[] }>(
    `/services/list${buildQuery({ scope: "agent", agent_id: agentId })}`,
  )
  return data.services
}

export async function getServiceInstance(addr: ServiceAddress): Promise<ServiceInstance> {
  return api<ServiceInstance>(svcPath(addr, ""))
}

export async function getServiceState(addr: ServiceAddress): Promise<ServiceState> {
  return api<ServiceState>(svcPath(addr, "/state"))
}

export async function getInstanceAuthStatus(addr: ServiceAddress): Promise<AuthStatusView> {
  const data = await api<{ auth: AuthStatusView }>(svcPath(addr, "/auth"))
  return data.auth
}

export async function startInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return api(svcPath(addr, "/auth/start"), { method: "POST" })
}

export async function refreshInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return api(svcPath(addr, "/auth/refresh"), { method: "POST" })
}

export async function logoutInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return api(svcPath(addr, "/auth/logout"), { method: "POST" })
}

export async function upgradeInstanceAuthorizationScope(
  addr: ServiceAddress,
  requiredScope: string,
): Promise<AuthOperationResult> {
  return api(svcPath(addr, "/auth/scope-upgrade"), {
    method: "POST",
    body: JSON.stringify({ required_scope: requiredScope }),
  })
}

export async function listInstanceTools(
  addr: ServiceAddress,
  filter: ToolVisibilityFilter = "available",
): Promise<ToolInfo[]> {
  const data = await api<{ tools: ToolInfo[] }>(svcPath(addr, "/tools/list", { filter }))
  return data.tools
}

export async function setInstanceToolPolicy(addr: ServiceAddress, availableTools: string[]) {
  return api(svcPath(addr, "/tool-policy"), {
    method: "PUT",
    body: JSON.stringify({ available_tools: availableTools }),
  })
}

export async function clearInstanceToolPolicy(addr: ServiceAddress) {
  return api(svcPath(addr, "/tool-policy"), { method: "DELETE" })
}

export async function listInstanceResources(addr: ServiceAddress): Promise<ResourceInfo[]> {
  const data = await api<{ resources: ResourceInfo[] }>(svcPath(addr, "/resources/list"))
  return data.resources
}

export async function listInstanceResourceTemplates(
  addr: ServiceAddress,
): Promise<ResourceTemplateInfo[]> {
  const data = await api<{ resource_templates: ResourceTemplateInfo[] }>(
    svcPath(addr, "/resources/templates"),
  )
  return data.resource_templates
}

export async function listInstancePrompts(addr: ServiceAddress): Promise<PromptInfo[]> {
  const data = await api<{ prompts: PromptInfo[] }>(svcPath(addr, "/prompts/list"))
  return data.prompts
}

export async function readInstanceResource(addr: ServiceAddress, uri: string) {
  return api(svcPath(addr, "/resources/read", { uri }))
}

export async function checkInstance(addr: ServiceAddress) {
  return api(svcPath(addr, "/check"))
}

export async function connectInstance(addr: ServiceAddress): Promise<ServiceState> {
  return api(svcPath(addr, "/connect"), { method: "POST" })
}

export async function disconnectInstance(addr: ServiceAddress) {
  return api(svcPath(addr, "/disconnect"), { method: "POST" })
}

export async function restartInstance(addr: ServiceAddress) {
  return api(svcPath(addr, "/restart"), { method: "POST" })
}

export async function callInstanceTool(
  addr: ServiceAddress,
  toolName: string,
  args: Record<string, unknown>,
) {
  return api(svcPath(addr, "/tools/call"), {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  })
}

export async function getInstancePrompt(
  addr: ServiceAddress,
  promptName: string,
  args: Record<string, unknown> = {},
) {
  return api(svcPath(addr, "/prompts/get"), {
    method: "POST",
    body: JSON.stringify({ prompt_name: promptName, args }),
  })
}
