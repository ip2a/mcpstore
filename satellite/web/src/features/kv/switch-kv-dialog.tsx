import { useEffect, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"
import { switchKv, type KvStoreName } from "@/lib/api"

const kvOptions: KvStoreName[] = ["memory", "redis"]

export function SwitchKvDialog({
  open,
  current,
  onOpenChange,
  onChanged,
}: {
  open: boolean
  current?: KvStoreName
  onOpenChange: (open: boolean) => void
  onChanged: () => Promise<void>
}) {
  const { t } = useI18n()
  const [target, setTarget] = useState<KvStoreName>(current || "memory")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open && current) setTarget(current)
  }, [current, open])

  async function onSwitch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    try {
      await switchKv(target)
      toast.success(t("kvStorageSwitched"))
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
          <DialogTitle>{t("switchKvStorage")}</DialogTitle>
        </DialogHeader>
        <DialogForm onSubmit={onSwitch}>
          <Field>
            <FieldLabel>{t("targetKvStorage")}</FieldLabel>
            <Select value={target} onValueChange={(value) => setTarget(value as KvStoreName)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {kvOptions.map((option) => (
                    <SelectItem key={option} value={option}>
                      {option}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <DialogFormFooter
            onCancel={() => onOpenChange(false)}
            submitLabel={submitting ? t("switching") : t("switch")}
            submitting={submitting}
          />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}
