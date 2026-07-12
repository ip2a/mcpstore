import { useEffect, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"
import { switchCache, type CacheBackend } from "@/lib/api"

const cacheOptions: CacheBackend[] = ["memory", "redis", "openkeyv_memory", "openkeyv_redis"]

export function SwitchCacheDialog({ open, current, onOpenChange, onChanged }: { open: boolean; current?: CacheBackend; onOpenChange: (open: boolean) => void; onChanged: () => Promise<void> }) {
  const { t } = useI18n()
  const [target, setTarget] = useState<CacheBackend>(current || "memory")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open && current) setTarget(current)
  }, [current, open])

  async function onSwitch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    try {
      await switchCache(target)
      toast.success(t("cacheStorageSwitched"))
      await onChanged()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("switchFailed"))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("switchCacheStorage")}</DialogTitle>
          <DialogDescription>{t("currentCacheStorage", { backend: current || t("unknown") })}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onSwitch}>
          <Field>
            <FieldLabel>{t("targetCacheStorage")}</FieldLabel>
            <Select value={target} onValueChange={(value) => setTarget(value as CacheBackend)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {cacheOptions.map((option) => <SelectItem key={option} value={option}>{option}</SelectItem>)}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <DialogFormFooter onCancel={() => onOpenChange(false)} submitLabel={submitting ? t("switching") : t("switch")} submitting={submitting} />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}
