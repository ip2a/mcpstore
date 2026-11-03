import { useEffect, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/lib/i18n-context"
import type { ScopeRef, ServiceOverrideRule } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { useDeleteServiceOverride, useServiceOverrideQuery, useSetServiceOverride } from "@/features/overrides/queries"

/**
 * Gateway-facing projection editor for one service instance: what the
 * aggregated MCP surface exposes (display name / description / icons) or
 * whether it exposes the service at all. No rule = pass-through.
 */
export function ServiceOverridePane({
  serviceName,
  scope,
}: {
  serviceName: string
  scope: ScopeRef
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const overrideQuery = useServiceOverrideQuery(serviceName, scope)
  const rule: ServiceOverrideRule | undefined = overrideQuery.data?.override ?? undefined
  const [displayName, setDisplayName] = useState("")
  const [description, setDescription] = useState("")
  const [enabled, setEnabled] = useState(false)
  useEffect(() => {
    setDisplayName(rule?.display_name || "")
    setDescription(rule?.description || "")
    setEnabled(rule?.enabled === false)
  }, [rule])

  const setOverride = useSetServiceOverride(serviceName, scope)
  const deleteOverride = useDeleteServiceOverride(serviceName, scope)
  const busy = setOverride.isPending || deleteOverride.isPending
  // Full-replace semantics: saving before the existing rule loads would wipe its fields.
  const loading = overrideQuery.isPending

  async function refresh() {
    await client.invalidateQueries({ queryKey: queryKeys.serviceOverride(serviceName, scope) })
    await client.invalidateQueries({ queryKey: queryKeys.instance({ service_name: serviceName, scope }) })
  }

  async function save() {
    const body: Record<string, unknown> = {}
    if (rule) {
      for (const field of ["meta", "annotations", "icons", "tags"] as const) {
        if (field in rule) body[field] = (rule as Record<string, unknown>)[field]
      }
    }
    if (displayName) body.display_name = displayName
    if (description) body.description = description
    // The switch means "disabled"; only enabled=Some(false) on the backend hides the service.
    body.enabled = !enabled
    await setOverride.mutateAsync(body)
    await refresh()
    toast.success(t("saved"))
  }

  async function resetToPassthrough() {
    await deleteOverride.mutateAsync()
    await refresh()
    toast.success(t("saved"))
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-muted-foreground">{t("serviceOverrideDescription")}</p>
      {!rule ? <p className="text-xs text-muted-foreground">{t("serviceOverridePassthroughHint")}</p> : null}
      <div className="grid gap-1.5">
        <Label htmlFor="service-override-name">{t("overrideDisplayName")}</Label>
        <Input
          id="service-override-name"
          value={displayName}
          placeholder={serviceName}
          onChange={(event) => setDisplayName(event.target.value)}
        />
        <p className="text-xs text-muted-foreground">{t("serviceOverrideDisplayNameHint")}</p>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="service-override-description">{t("overrideDescription")}</Label>
        <Textarea
          id="service-override-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
        />
      </div>
      <div className="flex items-center justify-between rounded-md border px-3 py-2">
        <div className="grid gap-0.5">
          <Label htmlFor="service-override-enabled">{t("serviceOverrideDisabled")}</Label>
          <p className="text-xs text-muted-foreground">{t("serviceOverrideDisabledHint")}</p>
        </div>
        <Switch
          id="service-override-enabled"
          checked={enabled}
          onCheckedChange={setEnabled}
        />
      </div>
      <div className="flex gap-2">
        <Button size="sm" onClick={() => void save()} disabled={busy || loading}>
          {busy ? t("saving") : t("saveOverride")}
        </Button>
        {rule ? (
          <Button size="sm" variant="outline" onClick={() => void resetToPassthrough()} disabled={busy}>
            {t("serviceOverrideReset")}
          </Button>
        ) : null}
      </div>
    </div>
  )
}
