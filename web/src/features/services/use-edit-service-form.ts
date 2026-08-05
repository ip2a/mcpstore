import { useEffect, useMemo, useState, type FormEvent } from "react"
import { toast } from "sonner"

import {
  fieldsToConfig,
  serviceInstanceToFields,
  type ServiceConfigFields,
  type ServiceConfigFormat,
} from "@/features/services/service-config-draft"
import { updateServiceScope } from "@/lib/api"
import type { ServiceInstance } from "@/lib/api"

export function useEditServiceForm({
  onCancel,
  onUpdated,
  service,
}: {
  onCancel: () => void
  onUpdated: () => Promise<void>
  service: ServiceInstance
}) {
  const initialFields = useMemo(() => serviceInstanceToFields(service), [service])
  const [configFields, setConfigFields] = useState<ServiceConfigFields>(initialFields)
  const [previewFormat, setPreviewFormat] = useState<ServiceConfigFormat>("json")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    setConfigFields(initialFields)
  }, [initialFields])

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    try {
      const config = fieldsToConfig(configFields)
      if (configFields.transport === "stdio" && !config.command) {
        throw new Error("stdio config requires command")
      }
      if (configFields.transport !== "stdio" && !config.url) {
        throw new Error("http config requires url")
      }

      await updateServiceScope({
        serviceName: service.service_name,
        scope: service.scope,
        transport: configFields.transport,
        commandOrUrl: "",
        config,
        handshakeMode: configFields.handshakeMode,
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
    configFields,
    onSubmit,
    previewFormat,
    setConfigFields,
    setPreviewFormat,
    submitting,
  }
}
