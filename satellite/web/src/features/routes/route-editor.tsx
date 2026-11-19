import { useEffect, useMemo, useState } from "react"
import { ChevronDownIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/lib/i18n-context"
import type {
  RouteFields,
  PromptRouteRule,
  ResourceRouteRule,
  ResourceTemplateRouteRule,
  ScopeRef,
  ToolArgumentRoute,
  ToolRouteRule,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { useQueryClient } from "@tanstack/react-query"
import {
  useDeletePromptRoute,
  useDeleteResourceRoute,
  useDeleteResourceTemplateRoute,
  useDeleteToolRoute,
  usePromptRoutesQuery,
  useResourceRoutesQuery,
  useResourceTemplateRoutesQuery,
  useSetPromptRoute,
  useSetResourceRoute,
  useSetResourceTemplateRoute,
  useSetToolRoute,
  useToolRoutesQuery,
} from "./queries"

type Kind = "tool" | "prompt" | "resource" | "resource_template"
type Props = {
  kind: Kind
  serviceName: string
  componentKey: string
  scope: ScopeRef
  existingRoute?: RouteFields
  onSuccess?: () => void
}

function sameScope(a: ScopeRef, b: ScopeRef) {
  return (
    a.type === b.type && (a.type === "store" || a.agent_id === (b as { agent_id: string }).agent_id)
  )
}
function ruleKey(
  item: ToolRouteRule | PromptRouteRule | ResourceRouteRule | ResourceTemplateRouteRule,
  kind: Kind,
) {
  if (kind === "tool" && "tool_name" in item) return item.tool_name
  if (kind === "prompt" && "prompt_name" in item) return item.prompt_name
  if (kind === "resource" && "uri" in item) return item.uri
  if (kind === "resource_template" && "uri_template" in item) return item.uri_template
  return ""
}

export function RouteEditor({
  kind,
  serviceName,
  componentKey,
  scope,
  existingRoute,
  onSuccess,
}: Props) {
  const { t } = useI18n()
  const client = useQueryClient()
  const tools = useToolRoutesQuery(kind === "tool")
  const prompts = usePromptRoutesQuery(kind === "prompt")
  const resources = useResourceRoutesQuery(kind === "resource")
  const templates = useResourceTemplateRoutesQuery(kind === "resource_template")
  const rules = useMemo(
    () =>
      ({
        tool: tools.data?.routes,
        prompt: prompts.data?.routes,
        resource: resources.data?.routes,
        resource_template: templates.data?.routes,
      })[kind] || [],
    [kind, tools.data, prompts.data, resources.data, templates.data],
  )
  const rule =
    existingRoute ||
    (
      rules as Array<
        ToolRouteRule | PromptRouteRule | ResourceRouteRule | ResourceTemplateRouteRule
      >
    ).find(
      (item) =>
        item.service_name === serviceName &&
        sameScope(item.scope, scope) &&
        ruleKey(item, kind) === componentKey,
    )
  const [open, setOpen] = useState(false)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [displayName, setDisplayName] = useState("")
  const [description, setDescription] = useState("")
  const [enabled, setEnabled] = useState(true)
  const [mimeType, setMimeType] = useState("")
  const [iconsText, setIconsText] = useState("")
  const [arguments_, setArguments] = useState<ToolArgumentRoute[]>([])
  useEffect(() => {
    setDisplayName(rule?.display_name || "")
    setDescription(rule?.description || "")
    setEnabled(rule?.enabled !== false)
    setMimeType(
      (rule as ResourceRouteRule | ResourceTemplateRouteRule | undefined)?.mime_type || "",
    )
    setIconsText(rule?.icons ? JSON.stringify(rule.icons) : "")
    setArguments(
      kind === "tool"
        ? ((rule as ToolRouteRule | undefined)?.arguments || []).map((argument) => ({
            ...argument,
          }))
        : [],
    )
  }, [rule, kind])
  const setTool = useSetToolRoute(serviceName, componentKey, scope)
  const delTool = useDeleteToolRoute(serviceName, componentKey, scope)
  const setPrompt = useSetPromptRoute(serviceName, componentKey, scope)
  const delPrompt = useDeletePromptRoute(serviceName, componentKey, scope)
  const setResource = useSetResourceRoute(serviceName, componentKey, scope)
  const delResource = useDeleteResourceRoute(serviceName, componentKey, scope)
  const setTemplate = useSetResourceTemplateRoute(serviceName, componentKey, scope)
  const delTemplate = useDeleteResourceTemplateRoute(serviceName, componentKey, scope)
  const busy = [
    setTool,
    setPrompt,
    setResource,
    setTemplate,
    delTool,
    delPrompt,
    delResource,
    delTemplate,
  ].some((mutation) => mutation.isPending)
  // Full-replace semantics: saving before the rules list loads would wipe preserved fields.
  const loading =
    (kind === "tool"
      ? tools.isPending
      : kind === "prompt"
        ? prompts.isPending
        : kind === "resource"
          ? resources.isPending
          : templates.isPending) && !existingRoute
  function patch() {
    const body: Record<string, unknown> = {}
    const supportedFields = [
      "display_name",
      "description",
      "meta",
      "annotations",
      "icons",
      "tags",
      "enabled",
    ]
    if (kind === "tool") supportedFields.push("arguments", "safety_policy")
    if (kind === "resource" || kind === "resource_template") supportedFields.push("mime_type")
    for (const field of supportedFields) {
      if (rule && field in rule) body[field] = (rule as Record<string, unknown>)[field]
    }
    if (displayName !== (rule?.display_name || "")) body.display_name = displayName
    if (description !== (rule?.description || "")) body.description = description
    if (enabled !== (rule?.enabled !== false)) body.enabled = enabled
    if (kind === "resource" || kind === "resource_template") {
      const old =
        (rule as ResourceRouteRule | ResourceTemplateRouteRule | undefined)?.mime_type || ""
      if (mimeType !== old) body.mime_type = mimeType
    }
    const oldIcons = rule?.icons ? JSON.stringify(rule.icons) : ""
    if (iconsText !== oldIcons) {
      if (!iconsText.trim()) delete body.icons
      else {
        try {
          body.icons = JSON.parse(iconsText)
        } catch {
          toast.error(t("invalidJson"))
          return null
        }
      }
    }
    if (kind === "tool") body.arguments = arguments_
    return body
  }
  async function refreshAffected() {
    await Promise.all([
      client.invalidateQueries({ queryKey: queryKeys.routes(kind) }),
      client.invalidateQueries({
        queryKey: queryKeys.instance({ service_name: serviceName, scope }),
      }),
    ])
  }
  async function save() {
    const body = patch()
    if (!body) return
    if (kind === "tool") await setTool.mutateAsync(body)
    else if (kind === "prompt") await setPrompt.mutateAsync(body)
    else if (kind === "resource") await setResource.mutateAsync(body)
    else await setTemplate.mutateAsync(body)
    await refreshAffected()
    toast.success(t("saved"))
    onSuccess?.()
  }
  async function remove() {
    if (kind === "tool") await delTool.mutateAsync()
    else if (kind === "prompt") await delPrompt.mutateAsync()
    else if (kind === "resource") await delResource.mutateAsync()
    else await delTemplate.mutateAsync()
    await refreshAffected()
    setOpen(false)
    toast.success(t("delete"))
    onSuccess?.()
  }
  return (
    <>
      <Collapsible open={open} onOpenChange={setOpen} className="border-b pb-4">
        <CollapsibleTrigger className="flex w-full items-center justify-between py-1 text-left">
          <span className="font-medium">{t("routeEditor")}</span>
          <ChevronDownIcon className="size-4 transition-transform data-[state=open]:rotate-180" />
        </CollapsibleTrigger>
        <CollapsibleContent className="space-y-4 pt-3">
          <div className="grid gap-3">
            <div className="grid gap-1.5">
              <Label htmlFor={`route-name-${componentKey}`}>{t("routeDisplayName")}</Label>
              <Input
                id={`route-name-${componentKey}`}
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor={`route-description-${componentKey}`}>
                {t("routeDescription")}
              </Label>
              <Textarea
                id={`route-description-${componentKey}`}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor={`route-icons-${componentKey}`}>{t("routeIcons")}</Label>
              <Textarea
                id={`route-icons-${componentKey}`}
                className="font-mono text-xs"
                placeholder='[{"src": "https://…/logo.svg", "mimeType": "image/svg+xml"}]'
                value={iconsText}
                onChange={(event) => setIconsText(event.target.value)}
              />
            </div>
            {kind === "resource" || kind === "resource_template" ? (
              <div className="grid gap-1.5">
                <Label htmlFor={`route-mime-${componentKey}`}>{t("routeMimeType")}</Label>
                <Input
                  id={`route-mime-${componentKey}`}
                  value={mimeType}
                  onChange={(event) => setMimeType(event.target.value)}
                />
              </div>
            ) : null}
            <div className="flex items-center justify-between rounded-md border px-3 py-2">
              <Label htmlFor={`route-enabled-${componentKey}`}>{t("routeEnabled")}</Label>
              <Switch
                id={`route-enabled-${componentKey}`}
                checked={enabled}
                onCheckedChange={setEnabled}
              />
            </div>
            {kind === "tool" ? (
              <RouteArgumentsEditor arguments_={arguments_} onChange={setArguments} />
            ) : null}
          </div>
          <div className="flex gap-2">
            <Button size="sm" onClick={() => void save()} disabled={busy || loading}>
              {busy ? t("saving") : t("saveRoute")}
            </Button>
            {rule ? (
              <Button
                size="sm"
                variant="outline"
                onClick={() => setDeleteOpen(true)}
                disabled={busy}
              >
                {t("deleteRoute")}
              </Button>
            ) : null}
          </div>
        </CollapsibleContent>
      </Collapsible>
      <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("deleteRoute")}</AlertDialogTitle>
            <AlertDialogDescription>{t("deleteConfirmDescription")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("cancel")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void remove()}>
              {t("delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

function RouteArgumentsEditor({
  arguments_,
  onChange,
}: {
  arguments_: ToolArgumentRoute[]
  onChange: (next: ToolArgumentRoute[]) => void
}) {
  const { t } = useI18n()
  function update(index: number, patch: Partial<ToolArgumentRoute>) {
    onChange(arguments_.map((argument, i) => (i === index ? { ...argument, ...patch } : argument)))
  }
  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between">
        <Label>{t("routeArguments")}</Label>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() =>
            onChange([
              ...arguments_,
              {
                original_name: "",
                new_name: null,
                hidden: false,
                default_value: null,
                description: null,
                validation_schema: null,
              },
            ])
          }
        >
          {t("addArgument")}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">{t("routeArgumentsHint")}</p>
      {arguments_.map((argument, index) => (
        <ArgumentRow
          key={argument.original_name ? String(argument.original_name) : `row-${index}`}
          argument={argument}
          onUpdate={(patch) => update(index, patch)}
          onDelete={() => onChange(arguments_.filter((_, i) => i !== index))}
        />
      ))}
    </div>
  )
}

function ArgumentRow({
  argument,
  onUpdate,
  onDelete,
}: {
  argument: ToolArgumentRoute
  onUpdate: (patch: Partial<ToolArgumentRoute>) => void
  onDelete: () => void
}) {
  const { t } = useI18n()
  // Raw text while typing; JSON parsed only on blur so keystrokes like "5." never corrupt the value.
  const [rawDefault, setRawDefault] = useState(
    argument.default_value == null ? "" : JSON.stringify(argument.default_value),
  )
  return (
    <div className="grid gap-2 rounded-md border p-2">
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1">
          <Label className="text-xs">{t("argOriginalName")}</Label>
          <Input
            className="h-8 font-mono text-xs"
            value={String(argument.original_name ?? "")}
            onChange={(event) => onUpdate({ original_name: event.target.value })}
          />
        </div>
        <div className="grid gap-1">
          <Label className="text-xs">{t("argNewName")}</Label>
          <Input
            className="h-8 font-mono text-xs"
            value={String(argument.new_name ?? "")}
            onChange={(event) => onUpdate({ new_name: event.target.value || null })}
          />
        </div>
      </div>
      <div className="grid grid-cols-[1fr_auto] items-end gap-2">
        <div className="grid gap-1">
          <Label className="text-xs">{t("argDefaultValue")}</Label>
          <Input
            className="h-8 font-mono text-xs"
            placeholder={t("argDefaultValuePlaceholder")}
            value={rawDefault}
            onChange={(event) => setRawDefault(event.target.value)}
            onBlur={() => {
              const text = rawDefault.trim()
              if (!text) {
                onUpdate({ default_value: null })
                return
              }
              try {
                onUpdate({ default_value: JSON.parse(text) })
              } catch {
                onUpdate({ default_value: text })
              }
            }}
          />
        </div>
        <div className="flex items-center gap-2 pb-1">
          <Switch
            id={`arg-hidden-${argument.original_name}`}
            checked={argument.hidden === true}
            onCheckedChange={(checked) => onUpdate({ hidden: checked })}
          />
          <Label htmlFor={`arg-hidden-${argument.original_name}`} className="text-xs">
            {t("argHidden")}
          </Label>
          <Button type="button" size="sm" variant="ghost" onClick={onDelete}>
            {t("delete")}
          </Button>
        </div>
      </div>
    </div>
  )
}
