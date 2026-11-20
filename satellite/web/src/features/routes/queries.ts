import { useMutation, useQuery } from "@tanstack/react-query"

import { api, buildQuery, scopeParams } from "@/lib/api/client"
import type {
  RouteFields,
  PromptRoutePatch,
  PromptRouteRule,
  ResourceRoutePatch,
  ResourceRouteRule,
  ResourceTemplateRoutePatch,
  ResourceTemplateRouteRule,
  ScopeRef,
  ServiceRoutePatch,
  ServiceRouteRule,
  ToolRoutePatch,
  ToolRouteRule,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type ListResult<T> = { routes: T[]; total: number }
type Kind = "tool" | "prompt" | "resource" | "resource_template"
const paths: Record<Kind, string> = {
  tool: "tool_routes",
  prompt: "prompt_routes",
  resource: "resource_routes",
  resource_template: "resource_template_routes",
}

function itemPath(kind: Kind, serviceName: string, key: string, scope?: ScopeRef) {
  // tool/prompt put the component name in the path (/services/:name/tool_routes/:tool_name),
  // resource/resource_template keys go in the query (?uri= / ?uri_template=).
  const scopeQ = scope ? buildQuery(scopeParams(scope)) : ""
  if (kind === "tool" || kind === "prompt") {
    return `/services/${encodeURIComponent(serviceName)}/${paths[kind]}/${encodeURIComponent(key)}${scopeQ}`
  }
  const componentQuery = kind === "resource" ? { uri: key } : { uri_template: key }
  return `/services/${encodeURIComponent(serviceName)}/${paths[kind]}${buildQuery({ ...componentQuery, ...(scope ? scopeParams(scope) : {}) })}`
}

async function list<T>(kind: Kind) {
  return api<ListResult<T>>(`/${paths[kind]}`)
}

function useList<T>(kind: Kind, enabled = true) {
  return useQuery({ queryKey: queryKeys.routes(kind), queryFn: () => list<T>(kind), enabled })
}

export const useToolRoutesQuery = (enabled = true) => useList<ToolRouteRule>("tool", enabled)
export const usePromptRoutesQuery = (enabled = true) => useList<PromptRouteRule>("prompt", enabled)
export const useResourceRoutesQuery = (enabled = true) =>
  useList<ResourceRouteRule>("resource", enabled)
export const useResourceTemplateRoutesQuery = (enabled = true) =>
  useList<ResourceTemplateRouteRule>("resource_template", enabled)

function useSet<T extends RouteFields>(
  kind: Kind,
  serviceName: string,
  key: string,
  scope?: ScopeRef,
) {
  return useMutation({
    mutationFn: (body: T) =>
      api(itemPath(kind, serviceName, key, scope), { method: "PUT", body: JSON.stringify(body) }),
  })
}
function useDelete(kind: Kind, serviceName: string, key: string, scope?: ScopeRef) {
  return useMutation({
    mutationFn: () => api(itemPath(kind, serviceName, key, scope), { method: "DELETE" }),
  })
}

export const useSetToolRoute = (serviceName: string, toolName: string, scope?: ScopeRef) =>
  useSet<ToolRoutePatch>("tool", serviceName, toolName, scope)
export const useDeleteToolRoute = (serviceName: string, toolName: string, scope?: ScopeRef) =>
  useDelete("tool", serviceName, toolName, scope)
export const useSetPromptRoute = (serviceName: string, name: string, scope?: ScopeRef) =>
  useSet<PromptRoutePatch>("prompt", serviceName, name, scope)
export const useDeletePromptRoute = (serviceName: string, name: string, scope?: ScopeRef) =>
  useDelete("prompt", serviceName, name, scope)
export const useSetResourceRoute = (serviceName: string, uri: string, scope?: ScopeRef) =>
  useSet<ResourceRoutePatch>("resource", serviceName, uri, scope)
export const useDeleteResourceRoute = (serviceName: string, uri: string, scope?: ScopeRef) =>
  useDelete("resource", serviceName, uri, scope)
export const useSetResourceTemplateRoute = (serviceName: string, uri: string, scope?: ScopeRef) =>
  useSet<ResourceTemplateRoutePatch>("resource_template", serviceName, uri, scope)
export const useDeleteResourceTemplateRoute = (
  serviceName: string,
  uri: string,
  scope?: ScopeRef,
) => useDelete("resource_template", serviceName, uri, scope)

// Service-level override: the gateway projects the service externally as display_name / a custom description, or disables it entirely (it disappears from the catalog).
export const useServiceRouteQuery = (serviceName: string, scope?: ScopeRef) =>
  useQuery({
    queryKey: queryKeys.serviceRoute(serviceName, scope),
    queryFn: () =>
      api<{ override: ServiceRouteRule | null }>(
        `/services/${encodeURIComponent(serviceName)}/service_route${scope ? buildQuery(scopeParams(scope)) : ""}`,
      ),
    enabled: Boolean(serviceName),
  })
export const useSetServiceRoute = (serviceName: string, scope?: ScopeRef) =>
  useMutation({
    mutationFn: (body: ServiceRoutePatch) =>
      api(
        `/services/${encodeURIComponent(serviceName)}/service_route${scope ? buildQuery(scopeParams(scope)) : ""}`,
        { method: "PUT", body: JSON.stringify(body) },
      ),
  })
export const useDeleteServiceRoute = (serviceName: string, scope?: ScopeRef) =>
  useMutation({
    mutationFn: () =>
      api(
        `/services/${encodeURIComponent(serviceName)}/service_route${scope ? buildQuery(scopeParams(scope)) : ""}`,
        { method: "DELETE" },
      ),
  })
