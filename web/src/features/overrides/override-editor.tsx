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
import type { ComponentOverrideCommon, PromptOverrideRule, ResourceOverrideRule, ResourceTemplateOverrideRule, ScopeRef, ToolOverrideRule } from "@/lib/api"
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
  const [open, setOpen] = useState(false); const [deleteOpen, setDeleteOpen] = useState(false); const [displayName, setDisplayName] = useState(""); const [description, setDescription] = useState(""); const [enabled, setEnabled] = useState(true); const [mimeType, setMimeType] = useState("")
  useEffect(() => { setDisplayName(rule?.display_name || ""); setDescription(rule?.description || ""); setEnabled(rule?.enabled !== false); setMimeType((rule as ResourceOverrideRule | ResourceTemplateOverrideRule | undefined)?.mime_type || "") }, [rule])
  const setTool = useSetToolOverride(serviceName, componentKey, scope); const delTool = useDeleteToolOverride(serviceName, componentKey, scope)
  const setPrompt = useSetPromptOverride(serviceName, componentKey, scope); const delPrompt = useDeletePromptOverride(serviceName, componentKey, scope)
  const setResource = useSetResourceOverride(serviceName, componentKey, scope); const delResource = useDeleteResourceOverride(serviceName, componentKey, scope)
  const setTemplate = useSetResourceTemplateOverride(serviceName, componentKey, scope); const delTemplate = useDeleteResourceTemplateOverride(serviceName, componentKey, scope)
  const busy = [setTool, setPrompt, setResource, setTemplate, delTool, delPrompt, delResource, delTemplate].some((mutation) => mutation.isPending)
  function patch() {
    const body: Record<string, unknown> = {}
    const supportedFields = ["display_name", "description", "meta", "annotations", "tags", "enabled"]
    if (kind === "tool") supportedFields.push("arguments", "safety_policy")
    if (kind === "resource" || kind === "resource_template") supportedFields.push("mime_type")
    for (const field of supportedFields) {
      if (rule && field in rule) body[field] = (rule as Record<string, unknown>)[field]
    }
    if (displayName !== (rule?.display_name || "")) body.display_name = displayName
    if (description !== (rule?.description || "")) body.description = description
    if (enabled !== (rule?.enabled !== false)) body.enabled = enabled
    if (kind === "resource" || kind === "resource_template") { const old = (rule as ResourceOverrideRule | ResourceTemplateOverrideRule | undefined)?.mime_type || ""; if (mimeType !== old) body.mime_type = mimeType }
    return body
  }
  async function refreshAffected() { await Promise.all([client.invalidateQueries({ queryKey: queryKeys.overrides(kind) }), client.invalidateQueries({ queryKey: queryKeys.instance({ service_name: serviceName, scope }) })]) }
  async function save() { const body = patch(); if (kind === "tool") await setTool.mutateAsync(body); else if (kind === "prompt") await setPrompt.mutateAsync(body); else if (kind === "resource") await setResource.mutateAsync(body); else await setTemplate.mutateAsync(body); await refreshAffected(); toast.success(t("saved")); onSuccess?.() }
  async function remove() { if (kind === "tool") await delTool.mutateAsync(); else if (kind === "prompt") await delPrompt.mutateAsync(); else if (kind === "resource") await delResource.mutateAsync(); else await delTemplate.mutateAsync(); await refreshAffected(); setOpen(false); toast.success(t("delete")); onSuccess?.() }
  return <><Collapsible open={open} onOpenChange={setOpen} className="border-b pb-4">
    <CollapsibleTrigger className="flex w-full items-center justify-between py-1 text-left"><span className="font-medium">{t("overrideEditor")}</span><ChevronDownIcon className="size-4 transition-transform data-[state=open]:rotate-180" /></CollapsibleTrigger>
    <CollapsibleContent className="space-y-4 pt-3"><div className="grid gap-3"><div className="grid gap-1.5"><Label htmlFor={`override-name-${componentKey}`}>{t("overrideDisplayName")}</Label><Input id={`override-name-${componentKey}`} value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></div><div className="grid gap-1.5"><Label htmlFor={`override-description-${componentKey}`}>{t("overrideDescription")}</Label><Textarea id={`override-description-${componentKey}`} value={description} onChange={(event) => setDescription(event.target.value)} /></div>{kind === "resource" || kind === "resource_template" ? <div className="grid gap-1.5"><Label htmlFor={`override-mime-${componentKey}`}>{t("overrideMimeType")}</Label><Input id={`override-mime-${componentKey}`} value={mimeType} onChange={(event) => setMimeType(event.target.value)} /></div> : null}<div className="flex items-center justify-between rounded-md border px-3 py-2"><Label htmlFor={`override-enabled-${componentKey}`}>{t("overrideEnabled")}</Label><Switch id={`override-enabled-${componentKey}`} checked={enabled} onCheckedChange={setEnabled} /></div>{kind === "tool" ? <p className="text-xs text-muted-foreground">{t("overrideAdvanced")} — arguments editor TBD.</p> : null}</div><div className="flex gap-2"><Button size="sm" onClick={() => void save()} disabled={busy}>{busy ? t("saving") : t("saveOverride")}</Button>{rule ? <Button size="sm" variant="outline" onClick={() => setDeleteOpen(true)} disabled={busy}>{t("deleteOverride")}</Button> : null}</div></CollapsibleContent>
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
