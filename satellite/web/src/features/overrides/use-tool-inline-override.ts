import { useMemo } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import {
  useSetToolOverride,
  useToolOverridesQuery,
} from "@/features/overrides/queries"
import type { ScopeRef, ToolInfo, ToolOverridePatch, ToolOverrideRule } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { getOriginalToolName } from "@/lib/tool-info"
import { queryKeys } from "@/lib/query-keys"

type ArgOverride = {
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

function asArgs(rule?: ToolOverrideRule | null): ArgOverride[] {
  return ((rule?.arguments || []) as ArgOverride[]).map((argument) => ({ ...argument }))
}

/** Full-replace body: keep fields the UI is not editing. */
function bodyFromRule(rule?: ToolOverrideRule | null): ToolOverridePatch {
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
  return body as ToolOverridePatch
}

function upsertArg(args: ArgOverride[], originalName: string, patch: Partial<ArgOverride>) {
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
  rule: ToolOverrideRule | null | undefined,
  displayedName: string,
) {
  const match = asArgs(rule).find((argument) => argument.new_name === displayedName)
  return match?.original_name || displayedName
}

export function useToolInlineOverride(
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
  const overridesQuery = useToolOverridesQuery(enabled)
  const setOverride = useSetToolOverride(serviceName || "", originalToolName || "", scope)
  const onDisplayNameApplied = options?.onDisplayNameApplied

  const rule = useMemo(() => {
    if (!enabled || !serviceName || !originalToolName || !scope) return null
    return (
      (overridesQuery.data?.overrides || []).find(
        (item) =>
          item.service_name === serviceName &&
          sameScope(item.scope, scope) &&
          item.tool_name === originalToolName,
      ) || null
    )
  }, [enabled, overridesQuery.data?.overrides, originalToolName, scope, serviceName])

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
      client.invalidateQueries({ queryKey: queryKeys.overrides("tool") }),
      client.invalidateQueries({
        queryKey: queryKeys.instance({ service_name: serviceName, scope }),
      }),
      // Tools catalog page uses the scope aggregate (`scopeTools`), not per-instance.
      client.invalidateQueries({ queryKey: queryKeys.scopes }),
    ])
  }

  async function commit(body: ToolOverridePatch) {
    await setOverride.mutateAsync(body)
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
    await setOverride.mutateAsync(body)
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
    await setOverride.mutateAsync(body)
    if (originalToolName) {
      const projectedName = rule?.display_name?.trim() || originalToolName
      patchToolsCache(projectedName, next)
    }
    await refresh()
    toast.success(t("saved"))
  }

  function patchOverridesCache(nextArgs: ArgOverride[], common?: ToolOverridePatch) {
    if (!serviceName || !originalToolName || !scope) return
    client.setQueriesData<{ overrides: ToolOverrideRule[]; total: number }>(
      { queryKey: queryKeys.overrides("tool") },
      (prev) => {
        const list = prev?.overrides ? [...prev.overrides] : []
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
              } as ToolOverrideRule)
        const nextRule: ToolOverrideRule = {
          ...base,
          ...common,
          arguments: nextArgs as ToolOverrideRule["arguments"],
          version: (base.version || 0) + 1,
          updated_at: Date.now() / 1000,
        }
        if (index >= 0) list[index] = nextRule
        else list.push(nextRule)
        return { overrides: list, total: list.length }
      },
    )
  }

  async function saveArgName(displayedName: string, next: string) {
    const originalName = resolveOriginalArgName(rule, displayedName)
    const trimmed = next.trim()
    if (!trimmed) {
      toast.error(t("overrideValueRequired"))
      return false
    }
    const args = upsertArg(asArgs(rule), originalName, {
      new_name: trimmed === originalName ? null : trimmed,
    })
    const body = bodyFromRule(rule)
    body.arguments = args
    await setOverride.mutateAsync(body)
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
    await setOverride.mutateAsync(body)
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
    busy: setOverride.isPending || (overridesQuery.isPending && !rule),
    nameOverridden: Boolean(rule?.display_name?.trim()),
    descriptionOverridden: rule?.description != null,
    argMeta,
    saveDisplayName,
    saveDescription,
    saveArgName,
    saveArgDescription,
  }
}
