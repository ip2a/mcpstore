import { useMemo } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import {
  useSetToolRoute,
  useToolRoutesQuery,
} from "@/features/routes/queries"
import type { ScopeRef, ToolInfo, ToolRoutePatch, ToolRouteRule } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { getOriginalToolName } from "@/lib/tool-info"
import { queryKeys } from "@/lib/query-keys"

type ArgRoute = {
  original_name: string
  new_name?: string | null
  hidden?: boolean
  default_value?: unknown
  description?: string | null
  validation_schema?: unknown
}

function sameScope(a: ScopeRef, b: ScopeRef) {
  return (
    a.type === b.type && (a.type === "store" || a.agent_id === (b as { agent_id: string }).agent_id)
  )
}

function asArgs(rule?: ToolRouteRule | null): ArgRoute[] {
  return ((rule?.arguments || []) as ArgRoute[]).map((argument) => ({ ...argument }))
}

/** Full-replace body: keep fields the UI is not editing. */
function bodyFromRule(rule?: ToolRouteRule | null): ToolRoutePatch {
  if (!rule) return {}
  const body: Record<string, unknown> = {}
  for (const field of [
    "display_name",
    "description",
    "meta",
    "annotations",
    "icons",
    "tags",
    "enabled",
    "arguments",
    "safety_policy",
  ] as const) {
    if (field in rule) body[field] = (rule as Record<string, unknown>)[field]
  }
  return body as ToolRoutePatch
}

function upsertArg(args: ArgRoute[], originalName: string, patch: Partial<ArgRoute>) {
  const index = args.findIndex((argument) => argument.original_name === originalName)
  if (index < 0) {
    return [
      ...args,
      {
        original_name: originalName,
        new_name: null,
        hidden: false,
        default_value: null,
        description: null,
        validation_schema: null,
        ...patch,
      },
    ]
  }
  return args.map((argument, i) => (i === index ? { ...argument, ...patch } : argument))
}

export function resolveOriginalArgName(
  rule: ToolRouteRule | null | undefined,
  displayedName: string,
) {
  const match = asArgs(rule).find((argument) => argument.new_name === displayedName)
  return match?.original_name || displayedName
}

export function useToolInlineRoute(
  serviceName: string | undefined,
  originalToolName: string | undefined,
  scope: ScopeRef | undefined,
  options?: {
    /** Called after a display_name save so the route can follow the projected name. */
    onDisplayNameApplied?: (projectedName: string) => void
  },
) {
  const { t } = useI18n()
  const client = useQueryClient()
  const enabled = Boolean(serviceName && originalToolName && scope)
  const routesQuery = useToolRoutesQuery(enabled)
  const setRoute = useSetToolRoute(serviceName || "", originalToolName || "", scope)
  const onDisplayNameApplied = options?.onDisplayNameApplied

  const rule = useMemo(() => {
    if (!enabled || !serviceName || !originalToolName || !scope) return null
    return (
      (routesQuery.data?.routes || []).find(
        (item) =>
          item.service_name === serviceName &&
          sameScope(item.scope, scope) &&
          item.tool_name === originalToolName,
      ) || null
    )
  }, [enabled, routesQuery.data?.routes, originalToolName, scope, serviceName])

  function patchToolsCache(projectedName: string, description?: string) {
    if (!serviceName || !scope || !originalToolName) return
    const addr = { service_name: serviceName, scope }
    client.setQueriesData<ToolInfo[]>({ queryKey: queryKeys.instanceTools(addr) }, (prev) => {
      if (!prev) return prev
      return prev.map((tool) => {
        if (getOriginalToolName(tool) !== originalToolName) return tool
        return {
          ...tool,
          name: projectedName,
          tool_name: originalToolName,
          ...(description !== undefined ? { description } : {}),
        }
      })
    })
  }

  async function refresh() {
    if (!serviceName || !scope) return
    await Promise.all([
      client.invalidateQueries({ queryKey: queryKeys.routes("tool") }),
      client.invalidateQueries({
        queryKey: queryKeys.instance({ service_name: serviceName, scope }),
      }),
      // Tools catalog page uses the scope aggregate (`scopeTools`), not per-instance.
      client.invalidateQueries({ queryKey: queryKeys.scopes }),
    ])
  }

  async function commit(body: ToolRoutePatch) {
    await setRoute.mutateAsync(body)
    await refresh()
    toast.success(t("saved"))
  }

  async function saveDisplayName(next: string) {
    if (!originalToolName) return
    const body = bodyFromRule(rule)
    const trimmed = next.trim()
    if (trimmed) body.display_name = trimmed
    else delete body.display_name
    const projectedName = trimmed || originalToolName
    await setRoute.mutateAsync(body)
    // Patch cache + rewrite route before refetch so the detail view never
    // briefly resolves to "tool not found" under the old URL key.
    patchToolsCache(projectedName)
    onDisplayNameApplied?.(projectedName)
    await refresh()
    toast.success(t("saved"))
  }

  async function saveDescription(next: string) {
    const body = bodyFromRule(rule)
    body.description = next
    await setRoute.mutateAsync(body)
    if (originalToolName) {
      const projectedName = rule?.display_name?.trim() || originalToolName
      patchToolsCache(projectedName, next)
    }
    await refresh()
    toast.success(t("saved"))
  }

  function patchOverridesCache(nextArgs: ArgRoute[], common?: ToolRoutePatch) {
    if (!serviceName || !originalToolName || !scope) return
    client.setQueriesData<{ routes: ToolRouteRule[]; total: number }>(
      { queryKey: queryKeys.routes("tool") },
      (prev) => {
        const list = prev?.routes ? [...prev.routes] : []
        const index = list.findIndex(
          (item) =>
            item.service_name === serviceName &&
            sameScope(item.scope, scope) &&
            item.tool_name === originalToolName,
        )
        const base =
          index >= 0
            ? list[index]
            : ({
                instance_id: "",
                service_name: serviceName,
                scope,
                tool_name: originalToolName,
                updated_at: Date.now() / 1000,
                version: 0,
              } as ToolRouteRule)
        const nextRule: ToolRouteRule = {
          ...base,
          ...common,
          arguments: nextArgs as ToolRouteRule["arguments"],
          version: (base.version || 0) + 1,
          updated_at: Date.now() / 1000,
        }
        if (index >= 0) list[index] = nextRule
        else list.push(nextRule)
        return { routes: list, total: list.length }
      },
    )
  }

  async function saveArgName(displayedName: string, next: string) {
    const originalName = resolveOriginalArgName(rule, displayedName)
    const trimmed = next.trim()
    if (!trimmed) {
      toast.error(t("routeValueRequired"))
      return false
    }
    const args = upsertArg(asArgs(rule), originalName, {
      new_name: trimmed === originalName ? null : trimmed,
    })
    const body = bodyFromRule(rule)
    body.arguments = args
    await setRoute.mutateAsync(body)
    // Keep original_name↔new_name mapping available immediately so prose
    // descriptions (keyed by original name) do not flash away after rename.
    patchOverridesCache(args, body)
    await refresh()
    toast.success(t("saved"))
  }

  async function saveArgDescription(displayedName: string, next: string) {
    const originalName = resolveOriginalArgName(rule, displayedName)
    const args = upsertArg(asArgs(rule), originalName, {
      description: next.trim() ? next : null,
    })
    const body = bodyFromRule(rule)
    body.arguments = args
    await setRoute.mutateAsync(body)
    patchOverridesCache(args, body)
    await refresh()
    toast.success(t("saved"))
  }

  function argMeta(displayedName: string) {
    const originalName = resolveOriginalArgName(rule, displayedName)
    const argument = asArgs(rule).find((item) => item.original_name === originalName)
    return {
      originalName,
      nameOverridden: Boolean(argument?.new_name),
      descriptionOverridden: argument?.description != null,
    }
  }

  return {
    enabled,
    rule,
    busy: setRoute.isPending || (routesQuery.isPending && !rule),
    nameOverridden: Boolean(rule?.display_name?.trim()),
    descriptionOverridden: rule?.description != null,
    argMeta,
    saveDisplayName,
    saveDescription,
    saveArgName,
    saveArgDescription,
  }
}
