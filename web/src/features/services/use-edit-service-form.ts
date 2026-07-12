import { useMemo, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { parseKvLines, updateService } from "@/lib/api"
import { getServiceEditValues } from "@/lib/service-info"
import type { AddServiceTransport } from "@/features/services/use-add-service-form"
import type { ServiceEntry } from "@/lib/api"

export function useEditServiceForm({
  onCancel,
  onUpdated,
  service,
}: {
  onCancel: () => void
  onUpdated: () => Promise<void>
  service: ServiceEntry
}) {
  const defaults = useMemo(() => getServiceEditValues(service), [service])
  const [transport, setTransport] = useState<AddServiceTransport>(defaults.transport)
  const [submitting, setSubmitting] = useState(false)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    setSubmitting(true)
    try {
      await updateService({
        name: service.name,
        transport,
        commandOrUrl: String(data.get("commandOrUrl") || "").trim(),
        description: String(data.get("description") || "").trim() || undefined,
        workingDir: String(data.get("workingDir") || "").trim() || undefined,
        env: parseKvLines(String(data.get("env") || "")),
        headers: parseKvLines(String(data.get("headers") || "")),
      })
      toast.success("Service updated")
      await onUpdated()
      onCancel()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Update service failed")
    } finally {
      setSubmitting(false)
    }
  }

  return {
    defaults,
    onSubmit,
    setTransport,
    submitting,
    transport,
  }
}
