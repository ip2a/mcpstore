import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"
import type { ScopeRef, ServiceRouteRule } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import {
  useDeleteServiceRoute,
  useServiceRouteQuery,
  useSetServiceRoute,
} from "@/features/routes/queries"

export type ServiceRoutePaneHandle = {
  save: () => Promise<void>
  discard: () => void
}

type GatewayVisibility = "show" | "hide"

/**
 * Gateway-facing projection editor for one service instance: rename / hide
 * (and preserve icons etc.). No rule = pass-through. Service-level description
 * is not edited here — it is not exposed on the MCP client catalog.
 */
export const ServiceRoutePane = forwardRef<
  ServiceRoutePaneHandle,
  {
    serviceName: string
    scope: ScopeRef
    /** When true, Save/Cancel live in the parent dialog footer. */
    hideActions?: boolean
    onBusyChange?: (busy: boolean) => void
  }
>(function ServiceRoutePane(
  { serviceName, scope, hideActions = false, onBusyChange },
  ref,
) {
  const { t } = useI18n()
  const client = useQueryClient()
  const routeQuery = useServiceRouteQuery(serviceName, scope)
  const rule: ServiceRouteRule | undefined = routeQuery.data?.route ?? undefined
  const [displayName, setDisplayName] = useState("")
  const [visibility, setVisibility] = useState<GatewayVisibility>("show")
  const draftRef = useRef({ displayName, visibility, rule })
  draftRef.current = { displayName, visibility, rule }

  function syncFromRule(next: ServiceRouteRule | undefined) {
    setDisplayName(next?.display_name || "")
    setVisibility(next?.enabled === false ? "hide" : "show")
  }

  useEffect(() => {
    syncFromRule(rule)
  }, [rule])

  const setRoute = useSetServiceRoute(serviceName, scope)
  const deleteRoute = useDeleteServiceRoute(serviceName, scope)
  const busy = setRoute.isPending || deleteRoute.isPending
  // Full-replace semantics: saving before the existing rule loads would wipe its fields.
  const loading = routeQuery.isPending

  useEffect(() => {
    onBusyChange?.(busy || loading)
  }, [busy, loading, onBusyChange])

  async function refresh() {
    await client.invalidateQueries({ queryKey: queryKeys.serviceRoute(serviceName, scope) })
    await client.invalidateQueries({
      queryKey: queryKeys.instance({ service_name: serviceName, scope }),
    })
    // List cards read display_name from /services/list enrichment — refresh those too.
    await client.invalidateQueries({ queryKey: queryKeys.instances, exact: true })
    await client.invalidateQueries({ queryKey: ["scopes"] })
  }

  async function saveDraft() {
    if (routeQuery.isPending) return
    const draft = draftRef.current
    const body: Record<string, unknown> = {}
    // Full-replace common: keep fields the UI does not edit.
    if (draft.rule) {
      for (const field of ["meta", "annotations", "icons", "tags", "description"] as const) {
        if (field in draft.rule) body[field] = (draft.rule as Record<string, unknown>)[field]
      }
    }
    if (draft.displayName) body.display_name = draft.displayName
    // Backend: enabled=false hides the service from the gateway catalog.
    body.enabled = draft.visibility === "show"
    await setRoute.mutateAsync(body)
    await refresh()
    toast.success(t("saved"))
  }

  async function resetToPassthrough() {
    await deleteRoute.mutateAsync()
    await refresh()
    toast.success(t("saved"))
  }

  useImperativeHandle(ref, () => ({
    save: saveDraft,
    discard: () => syncFromRule(draftRef.current.rule),
  }))

  return (
    <div className="flex flex-col gap-5">
      <FieldGroup className="gap-2">
        <Field
          orientation="horizontal"
          className="justify-between [&>[data-slot=field-label]]:flex-none"
        >
          <FieldLabel htmlFor="service-route-name">{t("routeDisplayName")}</FieldLabel>
          <Input
            id="service-route-name"
            className="w-52 max-w-[13rem]"
            value={displayName}
            placeholder={serviceName}
            onChange={(event) => setDisplayName(event.target.value)}
          />
        </Field>
        <span className="text-xs text-muted-foreground">{t("serviceRouteDisplayNameHint")}</span>
      </FieldGroup>

      <FieldGroup className="gap-2">
        <Field
          orientation="horizontal"
          className="justify-between [&>[data-slot=field-label]]:flex-none"
        >
          <FieldLabel htmlFor="service-route-visibility">
            {t("serviceRouteVisibility")}
          </FieldLabel>
          <Select
            value={visibility}
            onValueChange={(value) => setVisibility(value as GatewayVisibility)}
          >
            <SelectTrigger id="service-route-visibility" className="w-52 max-w-[13rem]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="show">{t("serviceRouteShow")}</SelectItem>
                <SelectItem value="hide">{t("serviceRouteHide")}</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
        <span className="text-xs text-muted-foreground">
          {visibility === "hide"
            ? t("serviceRouteHideHint")
            : t("serviceRouteShowHint")}
        </span>
      </FieldGroup>

      {hideActions ? (
        rule ? (
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="w-fit"
            onClick={() => void resetToPassthrough()}
            disabled={busy}
          >
            {t("serviceRouteReset")}
          </Button>
        ) : null
      ) : (
        <div className="flex gap-2">
          <Button
            type="button"
            size="sm"
            onClick={() => void saveDraft()}
            disabled={busy || loading}
          >
            {busy ? t("saving") : t("saveRoute")}
          </Button>
          {rule ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => void resetToPassthrough()}
              disabled={busy}
            >
              {t("serviceRouteReset")}
            </Button>
          ) : null}
        </div>
      )}
    </div>
  )
})
