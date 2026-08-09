import { useMutation, useQuery } from "@tanstack/react-query"

import { api, buildQuery, scopeParams } from "@/lib/api/client"
import type { ComponentOverrideCommon, PromptOverridePatch, PromptOverrideRule, ResourceOverridePatch, ResourceOverrideRule, ResourceTemplateOverridePatch, ResourceTemplateOverrideRule, ScopeRef, ToolOverridePatch, ToolOverrideRule } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

type ListResult<T> = { overrides: T[]; total: number }
type Kind = "tool" | "prompt" | "resource" | "resource_template"
const paths: Record<Kind, string> = {
  tool: "tool_overrides",
  prompt: "prompt_overrides",
  resource: "resource_overrides",
  resource_template: "resource_template_overrides",
}

function itemPath(kind: Kind, serviceName: string, key: string, scope?: ScopeRef) {
  const path = `/services/${encodeURIComponent(serviceName)}/${paths[kind]}`
  const componentQuery = kind === "resource"
    ? { uri: key }
    : kind === "resource_template"
      ? { uri_template: key }
      : {}
  return path + buildQuery({ ...componentQuery, ...(scope ? scopeParams(scope) : {}) })
}

async function list<T>(kind: Kind) {
  return api<ListResult<T>>(`/${paths[kind]}`)
}

function useList<T>(kind: Kind, enabled = true) {
  return useQuery({ queryKey: queryKeys.overrides(kind), queryFn: () => list<T>(kind), enabled })
}

export const useToolOverridesQuery = (enabled = true) => useList<ToolOverrideRule>("tool", enabled)
export const usePromptOverridesQuery = (enabled = true) => useList<PromptOverrideRule>("prompt", enabled)
export const useResourceOverridesQuery = (enabled = true) => useList<ResourceOverrideRule>("resource", enabled)
export const useResourceTemplateOverridesQuery = (enabled = true) => useList<ResourceTemplateOverrideRule>("resource_template", enabled)

function useSet<T extends ComponentOverrideCommon>(kind: Kind, serviceName: string, key: string, scope?: ScopeRef) {
  return useMutation({
    mutationFn: (body: T) => api(itemPath(kind, serviceName, key, scope), { method: "PUT", body: JSON.stringify(body) }),
  })
}
function useDelete(kind: Kind, serviceName: string, key: string, scope?: ScopeRef) {
  return useMutation({ mutationFn: () => api(itemPath(kind, serviceName, key, scope), { method: "DELETE" }) })
}

export const useSetToolOverride = (serviceName: string, toolName: string, scope?: ScopeRef) => useSet<ToolOverridePatch>("tool", serviceName, toolName, scope)
export const useDeleteToolOverride = (serviceName: string, toolName: string, scope?: ScopeRef) => useDelete("tool", serviceName, toolName, scope)
export const useSetPromptOverride = (serviceName: string, name: string, scope?: ScopeRef) => useSet<PromptOverridePatch>("prompt", serviceName, name, scope)
export const useDeletePromptOverride = (serviceName: string, name: string, scope?: ScopeRef) => useDelete("prompt", serviceName, name, scope)
export const useSetResourceOverride = (serviceName: string, uri: string, scope?: ScopeRef) => useSet<ResourceOverridePatch>("resource", serviceName, uri, scope)
export const useDeleteResourceOverride = (serviceName: string, uri: string, scope?: ScopeRef) => useDelete("resource", serviceName, uri, scope)
export const useSetResourceTemplateOverride = (serviceName: string, uri: string, scope?: ScopeRef) => useSet<ResourceTemplateOverridePatch>("resource_template", serviceName, uri, scope)
export const useDeleteResourceTemplateOverride = (serviceName: string, uri: string, scope?: ScopeRef) => useDelete("resource_template", serviceName, uri, scope)
