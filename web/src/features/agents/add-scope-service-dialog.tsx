import { useEffect } from "react"
import { PlusIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { useI18n } from "@/lib/i18n-context"

export function AddScopeServiceDialog({
  addableServiceNames,
  agentId,
  busy,
  onDeclareScope,
  onOpenChange,
  open,
  scopeServiceName,
  setScopeServiceName,
}: {
  addableServiceNames: string[]
  agentId: string
  busy: string | null
  onDeclareScope: (agentId: string, serviceName: string) => Promise<void>
  onOpenChange: (open: boolean) => void
  open: boolean
  scopeServiceName: string
  setScopeServiceName: (serviceName: string) => void
}) {
  const { t } = useI18n()
  const submitting = Boolean(busy?.startsWith("declare-scope:"))
  const canSubmit = Boolean(agentId && scopeServiceName && !submitting)

  useEffect(() => {
    if (!open) return
    if (!scopeServiceName && addableServiceNames[0]) {
      setScopeServiceName(addableServiceNames[0])
    }
  }, [addableServiceNames, open, scopeServiceName, setScopeServiceName])

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!agentId || !scopeServiceName) return
    await onDeclareScope(agentId, scopeServiceName)
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>{t("assignService")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <Field>
              <FieldLabel htmlFor="add-scope-agent">{t("scope")}</FieldLabel>
              <Input id="add-scope-agent" value={agentId} readOnly />
            </Field>
            <Field>
              <FieldLabel>{t("service")}</FieldLabel>
              {addableServiceNames.length ? (
                <Select
                  value={scopeServiceName || "none"}
                  onValueChange={(value) => setScopeServiceName(value === "none" ? "" : value)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {addableServiceNames.map((serviceName) => (
                        <SelectItem key={serviceName} value={serviceName}>
                          {serviceName}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              ) : (
                <Input value={t("none")} readOnly />
              )}
            </Field>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("cancel")}
            </Button>
            <Button type="submit" disabled={!canSubmit || !addableServiceNames.length}>
              {submitting ? <Spinner data-icon="inline-start" /> : <PlusIcon data-icon="inline-start" />}
              {submitting ? t("adding") : t("add")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
