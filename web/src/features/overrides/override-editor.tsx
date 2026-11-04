import { useEffect, useMemo, useState } from "react"
import { ChevronDownIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/lib/i18n-context"
import type { ComponentOverrideCommon, PromptOverrideRule, ResourceOverrideRule, ResourceTemplateOverrideRule, ScopeRef, ToolArgumentOverride, ToolOverrideRule } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { useQueryClient } from "@tanstack/react-query"
import { useDeletePromptOverride, useDeleteResourceOverride, useDeleteResourceTemplateOverride, useDeleteToolOverride, usePromptOverridesQuery, useResourceOverridesQuery, useResourceTemplateOverridesQuery, useSetPromptOverride, useSetResourceOverride, useSetResourceTemplateOverride, useSetToolOverride, useToolOverridesQuery } from "./queries"

type Kind = "tool" | "prompt" | "resource" | "resource_template"
type Props = { kind: Kind; serviceName: string; componentKey: string; scope: ScopeRef; existingOverride?: ComponentOverrideCommon; onSuccess?: () => void }

function sameScope(a: ScopeRef, b: ScopeRef) { return a.type === b.type && (a.type === "store" || a.agent_id === (b as { agent_id: string }).agent_id) }
function ruleKey(item: ToolOverrideRule | PromptOverrideRule | ResourceOverrideRule | ResourceTemplateOverrideRule, kind: Kind) {
  if (kind === "tool" && "tool_name" in item) return item.tool_name
  if (kind === "prompt" && "prompt_name" in item) return item.prompt_name
  if (kind === "resource" && "uri" in item) return item.uri
  if (kind === "resource_template" && "uri_template" in item) return item.uri_template
  return ""
}

export function OverrideEditor({ kind, serviceName, componentKey, scope, existingOverride, onSuccess }: Props) {
  const { t } = useI18n(); const client = useQueryClient()
  const tools = useToolOverridesQuery(kind === "tool"); const prompts = usePromptOverridesQuery(kind === "prompt"); const resources = useResourceOverridesQuery(kind === "resource"); const templates = useResourceTemplateOverridesQuery(kind === "resource_template")
  const rules = useMemo(() => ({ tool: tools.data?.overrides, prompt: prompts.data?.overrides, resource: resources.data?.overrides, resource_template: templates.data?.overrides }[kind] || []), [kind, tools.data, prompts.data, resources.data, templates.data])
  const rule = existingOverride || (rules as Array<ToolOverrideRule | PromptOverrideRule | ResourceOverrideRule | ResourceTemplateOverrideRule>).find((item) => item.service_name === serviceName && sameScope(item.scope, scope) && ruleKey(item, kind) === componentKey)
  const [open, setOpen] = useState(false); const [deleteOpen, setDeleteOpen] = useState(false); const [displayName, setDisplayName] = useState(""); const [description, setDescription] = useState(""); const [enabled, setEnabled] = useState(true); const [mimeType, setMimeType] = useState(""); const [iconsText, setIconsText] = useState(""); const [arguments_, setArguments] = useState<ToolArgumentOverride[]>([])
  useEffect(() => {
    setDisplayName(rule?.display_name || ""); setDescription(rule?.description || ""); setEnabled(rule?.enabled !== false); setMimeType((rule as ResourceOverrideRule | ResourceTemplateOverrideRule | undefined)?.mime_type || "")
    setIconsText(rule?.icons ? JSON.stringify(rule.icons) : "")
    setArguments(kind === "tool" ? ((rule as ToolOverrideRule | undefined)?.arguments || []).map((argument) => ({ ...argument })) : [])
  }, [rule, kind])
  const setTool = useSetToolOverride(serviceName, componentKey, scope); const delTool = useDeleteToolOverride(serviceName, componentKey, scope)
  const setPrompt = useSetPromptOverride(serviceName, componentKey, scope); const delPrompt = useDeletePromptOverride(serviceName, componentKey, scope)
  const setResource = useSetResourceOverride(serviceName, componentKey, scope); const delResource = useDeleteResourceOverride(serviceName, componentKey, scope)
  const setTemplate = useSetResourceTemplateOverride(serviceName, componentKey, scope); const delTemplate = useDeleteResourceTemplateOverride(serviceName, componentKey, scope)
  const busy = [setTool, setPrompt, setResource, setTemplate, delTool, delPrompt, delResource, delTemplate].some((mutation) => mutation.isPending)
  // Full-replace semantics: saving before the rules list loads would wipe preserved fields.
  const loading = (kind === "tool" ? tools.isPending : kind === "prompt" ? prompts.isPending : kind === "resource" ? resources.isPending : templates.isPending) && !existingOverride
  function patch() {
    const body: Record<string, unknown> = {}
    const supportedFields = ["display_name", "description", "meta", "annotations", "icons", "tags", "enabled"]
    if (kind === "tool") supportedFields.push("arguments", "safety_policy")
    if (kind === "resource" || kind === "resource_template") supportedFields.push("mime_type")
    for (const field of supportedFields) {
      if (rule && field in rule) body[field] = (rule as Record<string, unknown>)[field]
    }
    if (displayName !== (rule?.display_name || "")) body.display_name = displayName
    if (description !== (rule?.description || "")) body.description = description
    if (enabled !== (rule?.enabled !== false)) body.enabled = enabled
    if (kind === "resource" || kind === "resource_template") { const old = (rule as ResourceOverrideRule | ResourceTemplateOverrideRule | undefined)?.mime_type || ""; if (mimeType !== old) body.mime_type = mimeType }
    const oldIcons = rule?.icons ? JSON.stringify(rule.icons) : ""
    if (iconsText !== oldIcons) {
      if (!iconsText.trim()) delete body.icons
      else { try { body.icons = JSON.parse(iconsText) } catch { toast.error(t("invalidJson")); return null } }
    }
    if (kind === "tool") body.arguments = arguments_
    return body
  }
  async function refreshAffected() { await Promise.all([client.invalidateQueries({ queryKey: queryKeys.overrides(kind) }), client.invalidateQueries({ queryKey: queryKeys.instance({ service_name: serviceName, scope }) })]) }
  async function save() { const body = patch(); if (!body) return; if (kind === "tool") await setTool.mutateAsync(body); else if (kind === "prompt") await setPrompt.mutateAsync(body); else if (kind === "resource") await setResource.mutateAsync(body); else await setTemplate.mutateAsync(body); await refreshAffected(); toast.success(t("saved")); onSuccess?.() }
  async function remove() { if (kind === "tool") await delTool.mutateAsync(); else if (kind === "prompt") await delPrompt.mutateAsync(); else if (kind === "resource") await delResource.mutateAsync(); else await delTemplate.mutateAsync(); await refreshAffected(); setOpen(false); toast.success(t("delete")); onSuccess?.() }
  return <><Collapsible open={open} onOpenChange={setOpen} className="border-b pb-4">
    <CollapsibleTrigger className="flex w-full items-center justify-between py-1 text-left"><span className="font-medium">{t("overrideEditor")}</span><ChevronDownIcon className="size-4 transition-transform data-[state=open]:rotate-180" /></CollapsibleTrigger>
    <CollapsibleContent className="space-y-4 pt-3"><div className="grid gap-3"><div className="grid gap-1.5"><Label htmlFor={`override-name-${componentKey}`}>{t("overrideDisplayName")}</Label><Input id={`override-name-${componentKey}`} value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></div><div className="grid gap-1.5"><Label htmlFor={`override-description-${componentKey}`}>{t("overrideDescription")}</Label><Textarea id={`override-description-${componentKey}`} value={description} onChange={(event) => setDescription(event.target.value)} /></div><div className="grid gap-1.5"><Label htmlFor={`override-icons-${componentKey}`}>{t("overrideIcons")}</Label><Textarea id={`override-icons-${componentKey}`} className="font-mono text-xs" placeholder='[{"src": "https://…/logo.svg", "mimeType": "image/svg+xml"}]' value={iconsText} onChange={(event) => setIconsText(event.target.value)} /></div>{kind === "resource" || kind === "resource_template" ? <div className="grid gap-1.5"><Label htmlFor={`override-mime-${componentKey}`}>{t("overrideMimeType")}</Label><Input id={`override-mime-${componentKey}`} value={mimeType} onChange={(event) => setMimeType(event.target.value)} /></div> : null}<div className="flex items-center justify-between rounded-md border px-3 py-2"><Label htmlFor={`override-enabled-${componentKey}`}>{t("overrideEnabled")}</Label><Switch id={`override-enabled-${componentKey}`} checked={enabled} onCheckedChange={setEnabled} /></div>{kind === "tool" ? <OverrideArgumentsEditor arguments_={arguments_} onChange={setArguments} /> : null}</div><div className="flex gap-2"><Button size="sm" onClick={() => void save()} disabled={busy || loading}>{busy ? t("saving") : t("saveOverride")}</Button>{rule ? <Button size="sm" variant="outline" onClick={() => setDeleteOpen(true)} disabled={busy}>{t("deleteOverride")}</Button> : null}</div></CollapsibleContent>
  </Collapsible>
    <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("deleteOverride")}</AlertDialogTitle>
          <AlertDialogDescription>{t("deleteConfirmDescription")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("cancel")}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => void remove()}>{t("delete")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </>
}

function OverrideArgumentsEditor({ arguments_, onChange }: { arguments_: ToolArgumentOverride[]; onChange: (next: ToolArgumentOverride[]) => void }) {
  const { t } = useI18n()
  function update(index: number, patch: Partial<ToolArgumentOverride>) {
    onChange(arguments_.map((argument, i) => (i === index ? { ...argument, ...patch } : argument)))
  }
  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between">
        <Label>{t("overrideArguments")}</Label>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => onChange([...arguments_, { original_name: "", new_name: null, hidden: false, default_value: null, description: null, validation_schema: null }])}
        >
          {t("addArgument")}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">{t("overrideArgumentsHint")}</p>
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
  argument: ToolArgumentOverride
  onUpdate: (patch: Partial<ToolArgumentOverride>) => void
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
          <Input className="h-8 font-mono text-xs" value={String(argument.original_name ?? "")} onChange={(event) => onUpdate({ original_name: event.target.value })} />
        </div>
        <div className="grid gap-1">
          <Label className="text-xs">{t("argNewName")}</Label>
          <Input className="h-8 font-mono text-xs" value={String(argument.new_name ?? "")} onChange={(event) => onUpdate({ new_name: event.target.value || null })} />
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
              if (!text) { onUpdate({ default_value: null }); return }
              try { onUpdate({ default_value: JSON.parse(text) }) } catch { onUpdate({ default_value: text }) }
            }}
          />
        </div>
        <div className="flex items-center gap-2 pb-1">
          <Switch id={`arg-hidden-${argument.original_name}`} checked={argument.hidden === true} onCheckedChange={(checked) => onUpdate({ hidden: checked })} />
          <Label htmlFor={`arg-hidden-${argument.original_name}`} className="text-xs">{t("argHidden")}</Label>
          <Button type="button" size="sm" variant="ghost" onClick={onDelete}>
            {t("delete")}
          </Button>
        </div>
      </div>
    </div>
  )
}
