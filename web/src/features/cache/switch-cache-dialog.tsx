import { useEffect, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { switchCache, type CacheBackend } from "@/lib/api"

const cacheOptions: CacheBackend[] = ["memory", "redis", "openkeyv_memory", "openkeyv_redis"]

export function SwitchCacheDialog({ open, current, onOpenChange, onChanged }: { open: boolean; current?: CacheBackend; onOpenChange: (open: boolean) => void; onChanged: () => Promise<void> }) {
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
      toast.success("Cache storage switched")
      await onChanged()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Switch failed")
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Switch cache storage</DialogTitle>
          <DialogDescription>Current cache storage: {current || "unknown"}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onSwitch}>
          <Field>
            <FieldLabel>Target cache storage</FieldLabel>
            <Select value={target} onValueChange={(value) => setTarget(value as CacheBackend)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {cacheOptions.map((option) => <SelectItem key={option} value={option}>{option}</SelectItem>)}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <DialogFormFooter cancelLabel="Cancel" onCancel={() => onOpenChange(false)} submitLabel={submitting ? "Switching" : "Switch"} submitting={submitting} />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}
