import { useEffect, useMemo, useState } from "react"
import { PlusIcon } from "lucide-react"

import { AgentIdPicker } from "@/components/shared/agent-id-picker"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { getAgentId } from "@/features/agents/model"
import type { AgentItem, ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

function scopedAgentIds(services: ServiceInstance[], serviceName: string) {
  return new Set(
    services
      .filter((service) => service.service_name === serviceName && service.scope.type === "agent")
      .map((service) => (service.scope.type === "agent" ? service.scope.agent_id : ""))
      .filter(Boolean),
  )
}

export function AddServiceScopeDialog({
  agents,
  allServices,
  busy,
  onDeclareScope,
  onOpenChange,
  service,
}: {
  agents: AgentItem[]
  allServices: ServiceInstance[]
  busy: string | null
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
  onOpenChange: (open: boolean) => void
  service: ServiceInstance | null
}) {
  const { t } = useI18n()
  const [agentId, setAgentId] = useState("")
  const agentIds = useMemo(() => agents.map(getAgentId).filter(Boolean), [agents])
  const existingScopeAgentIds = useMemo(
    () => (service ? scopedAgentIds(allServices, service.service_name) : new Set<string>()),
    [allServices, service],
  )
  const selectableAgentIds = useMemo(
    () => agentIds.filter((id) => !existingScopeAgentIds.has(id)),
    [agentIds, existingScopeAgentIds],
  )
  const submitting = Boolean(busy?.startsWith("declare-scope:"))
  const trimmedAgentId = agentId.trim()
  const alreadyScoped = trimmedAgentId ? existingScopeAgentIds.has(trimmedAgentId) : false
  const canSubmit = Boolean(service && trimmedAgentId && !alreadyScoped && !submitting)

  useEffect(() => {
    if (!service) {
      setAgentId("")
      return
    }
    setAgentId(selectableAgentIds[0] || "")
  }, [selectableAgentIds, service?.service_name])

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!service || !trimmedAgentId || alreadyScoped) return
    await onDeclareScope(trimmedAgentId, service.service_name)
    onOpenChange(false)
  }

  return (
    <Dialog open={Boolean(service)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>{t("addServiceScope")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <Field>
              <FieldLabel htmlFor="add-scope-service">{t("service")}</FieldLabel>
              <Input id="add-scope-service" value={service?.service_name || ""} readOnly />
            </Field>
            <Field>
              <FieldLabel htmlFor="add-scope-agent-id">{t("agentId")}</FieldLabel>
              <AgentIdPicker
                id="add-scope-agent-id"
                agentIds={agentIds}
                value={agentId}
                onChange={setAgentId}
              />
              {alreadyScoped ? (
                <p className="mt-1 text-xs text-destructive">{t("addServiceScopeAlreadyExists")}</p>
              ) : null}
            </Field>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("cancel")}
            </Button>
            <Button type="submit" disabled={!canSubmit}>
              {submitting ? <Spinner data-icon="inline-start" /> : <PlusIcon data-icon="inline-start" />}
              {submitting ? t("adding") : t("addScope")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
