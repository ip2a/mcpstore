import { useState, type FormEvent } from "react"
import { toast } from "sonner"

import { addServiceFromConfig, type ServiceRestartPolicy, type ServiceStartupPolicy } from "@/lib/api"
import { buildServiceLifecycleConfig } from "@/features/services/service-lifecycle"
import {
  DEFAULT_SERVICE_CONFIG_FIELDS,
  fieldsToConfig,
  type ServiceConfigFields,
  type ServiceConfigFormat,
  type ServiceConfigTransport,
} from "@/features/services/service-config-draft"

export type AddServiceScope = "store" | "agent"
export type AddServiceTransport = ServiceConfigTransport

export function useAddServiceForm({ onAdded, onBack }: { onAdded: () => Promise<void>; onBack: () => void }) {
  const [scope, setScope] = useState<AddServiceScope>("store")
  const [serviceName, setServiceName] = useState("")
  const [configFields, setConfigFields] = useState<ServiceConfigFields>(DEFAULT_SERVICE_CONFIG_FIELDS)
  const [previewFormat, setPreviewFormat] = useState<ServiceConfigFormat>("json")
  const [agentId, setAgentId] = useState("")
  const [startupPolicy, setStartupPolicy] = useState<ServiceStartupPolicy>("lazy")
  const [restartPolicy, setRestartPolicy] = useState<ServiceRestartPolicy>("no")
  const [submitting, setSubmitting] = useState(false)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    setSubmitting(true)
    try {
      const name = serviceName.trim() || String(data.get("name") || "").trim()
      if (!name) {
        throw new Error("Service name is required")
      }
      const scopeRef =
        scope === "store"
          ? ({ type: "store" } as const)
          : ({
              type: "agent",
              agent_id: (agentId || String(data.get("agentId") || "").trim()),
            } as const)
      if (scopeRef.type === "agent" && !scopeRef.agent_id) {
        throw new Error("Agent ID is required")
      }
      const lifecycle = buildServiceLifecycleConfig({ startupPolicy, restartPolicy })
      const config = fieldsToConfig(configFields)

      if (config.transport === "stdio" && !config.command) {
        throw new Error("stdio config requires command")
      }
      if (config.transport !== "stdio" && !config.url) {
        throw new Error("http config requires url")
      }

      await addServiceFromConfig({ name, scope: scopeRef, config, lifecycle })
      toast.success("Service added")
      await onAdded()
      onBack()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Add service failed")
    } finally {
      setSubmitting(false)
    }
  }

  return {
    agentId,
    configFields,
    onSubmit,
    previewFormat,
    restartPolicy,
    scope,
    serviceName,
    setAgentId,
    setConfigFields,
    setPreviewFormat,
    setRestartPolicy,
    setScope,
    setServiceName,
    setStartupPolicy,
    startupPolicy,
    submitting,
  }
}
