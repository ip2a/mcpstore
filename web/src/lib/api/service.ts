import { apiUrl, buildQuery, readJson, request, scopeParams, scopeQuery } from "./client";
import type {
  AgentItem,
  AuthOperationResult,
  AuthStatusView,
  CacheBackend,
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
} from "../api";

/** 拼 `/services/:name<suffix>` + 作用域 query（+ 可选额外参数）。文档 §17。 */
function svcPath(
  addr: ServiceAddress,
  suffix: string,
  extra?: Record<string, string | number | boolean | null | undefined>,
): string {
  const base = `/services/${encodeURIComponent(addr.service_name)}${suffix}`;
  return extra
    ? `${base}${buildQuery({ ...scopeParams(addr.scope), ...extra })}`
    : `${base}${scopeQuery(addr.scope)}`;
}

export async function health() {
  return readJson<{ status: string; backend: CacheBackend }>(
    await fetch(apiUrl("/health")),
  );
}

export async function listServices(
  view: ScopeView = { type: "store" },
): Promise<ServiceInstance[]> {
  const query =
    view.type === "agent"
      ? buildQuery({ scope: "agent", agent_id: view.agent_id })
      : buildQuery({ scope: view.type });
  const data = await request<{ services: ServiceInstance[] }>(
    `/services/list${query}`,
  );
  return data.services;
}

/** 作用域注册表（root + store + 各 agent，每项带 service_count）。文档 §17.3。 */
export async function listScopes(): Promise<ScopeSummary[]> {
  const data = await request<{ scopes: ScopeSummary[] }>("/scopes/list");
  return data.scopes;
}

export async function listAgents(): Promise<AgentItem[]> {
  const data = await request<{ agents: AgentItem[] }>("/agents/list");
  return data.agents;
}

export async function listAgentServices(
  agentId: string,
): Promise<ServiceInstance[]> {
  const data = await request<{ services: ServiceInstance[] }>(
    `/services/list${buildQuery({ scope: "agent", agent_id: agentId })}`,
  );
  return data.services;
}

export async function getServiceInstance(
  addr: ServiceAddress,
): Promise<ServiceInstance> {
  return request<ServiceInstance>(svcPath(addr, ""));
}

export async function getServiceState(
  addr: ServiceAddress,
): Promise<ServiceState> {
  return request<ServiceState>(svcPath(addr, "/state"));
}

export async function getInstanceAuthStatus(
  addr: ServiceAddress,
): Promise<AuthStatusView> {
  const data = await request<{ auth: AuthStatusView }>(svcPath(addr, "/auth"));
  return data.auth;
}

export async function startInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return request(svcPath(addr, "/auth/start"), { method: "POST" });
}

export async function refreshInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return request(svcPath(addr, "/auth/refresh"), { method: "POST" });
}

export async function logoutInstanceAuthorization(
  addr: ServiceAddress,
): Promise<AuthOperationResult> {
  return request(svcPath(addr, "/auth/logout"), { method: "POST" });
}

export async function upgradeInstanceAuthorizationScope(
  addr: ServiceAddress,
  requiredScope: string,
): Promise<AuthOperationResult> {
  return request(svcPath(addr, "/auth/scope-upgrade"), {
    method: "POST",
    body: JSON.stringify({ required_scope: requiredScope }),
  });
}

export async function listInstanceTools(
  addr: ServiceAddress,
  filter: ToolVisibilityFilter = "available",
): Promise<ToolInfo[]> {
  const data = await request<{ tools: ToolInfo[] }>(
    svcPath(addr, "/tools/list", { filter }),
  );
  return data.tools;
}

export async function setInstanceToolPolicy(
  addr: ServiceAddress,
  availableTools: string[],
) {
  return request(svcPath(addr, "/tool-policy"), {
    method: "PUT",
    body: JSON.stringify({ available_tools: availableTools }),
  });
}

export async function clearInstanceToolPolicy(addr: ServiceAddress) {
  return request(svcPath(addr, "/tool-policy"), { method: "DELETE" });
}

export async function listInstanceResources(
  addr: ServiceAddress,
): Promise<ResourceInfo[]> {
  const data = await request<{ resources: ResourceInfo[] }>(
    svcPath(addr, "/resources/list"),
  );
  return data.resources;
}

export async function listInstanceResourceTemplates(
  addr: ServiceAddress,
): Promise<ResourceTemplateInfo[]> {
  const data = await request<{ resource_templates: ResourceTemplateInfo[] }>(
    svcPath(addr, "/resources/templates"),
  );
  return data.resource_templates;
}

export async function listInstancePrompts(
  addr: ServiceAddress,
): Promise<PromptInfo[]> {
  const data = await request<{ prompts: PromptInfo[] }>(
    svcPath(addr, "/prompts/list"),
  );
  return data.prompts;
}

export async function readInstanceResource(addr: ServiceAddress, uri: string) {
  return request(svcPath(addr, "/resources/read", { uri }));
}

export async function checkInstance(addr: ServiceAddress) {
  return request(svcPath(addr, "/check"));
}

export async function connectInstance(addr: ServiceAddress) {
  return request(svcPath(addr, "/connect"), { method: "POST" });
}

export async function disconnectInstance(addr: ServiceAddress) {
  return request(svcPath(addr, "/disconnect"), { method: "POST" });
}

export async function restartInstance(addr: ServiceAddress) {
  return request(svcPath(addr, "/restart"), { method: "POST" });
}

export async function callInstanceTool(
  addr: ServiceAddress,
  toolName: string,
  args: Record<string, unknown>,
) {
  return request(svcPath(addr, "/tools/call"), {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  });
}
