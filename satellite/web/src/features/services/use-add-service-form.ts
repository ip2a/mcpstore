import { useState, type FormEvent } from "react"
import { toast } from "sonner"

import {
  addServiceFromConfig,
  connectInstance,
  type ScopeView,
  type ServiceRestartPolicy,
  type ServiceStartupPolicy,
} from "@/lib/api"
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
export type AddServiceIntent = "add" | "add-connect"

function defaultsFromScope(initialScope?: ScopeView): {
  scope: AddServiceScope
  agentId: string
} {
  if (initialScope?.type === "agent" && initialScope.agent_id.trim()) {
    return { scope: "agent", agentId: initialScope.agent_id.trim() }
  }
  return { scope: "store", agentId: "" }
}

function resolveIntent(event: FormEvent<HTMLFormElement>): AddServiceIntent {
  const submitter = (event.nativeEvent as SubmitEvent).submitter
  if (submitter instanceof HTMLButtonElement && submitter.value === "add-connect") {
    return "add-connect"
  }
  const data = new FormData(event.currentTarget)
  return data.get("intent") === "add-connect" ? "add-connect" : "add"
}

export function useAddServiceForm({
  initialScope,
  onAdded,
  onBack,
}: {
  initialScope?: ScopeView
  onAdded: () => Promise<void>
  onBack: () => void
}) {
  const initial = defaultsFromScope(initialScope)
  const [scope, setScope] = useState<AddServiceScope>(initial.scope)
  const [serviceName, setServiceName] = useState("")
  const [configFields, setConfigFields] = useState<ServiceConfigFields>(
    DEFAULT_SERVICE_CONFIG_FIELDS,
  )
  const [previewFormat, setPreviewFormat] = useState<ServiceConfigFormat>("json")
  const [agentId, setAgentId] = useState(initial.agentId)
  const [startupPolicy, setStartupPolicy] = useState<ServiceStartupPolicy>("lazy")
  const [restartPolicy, setRestartPolicy] = useState<ServiceRestartPolicy>("no")
  const [submittingIntent, setSubmittingIntent] = useState<AddServiceIntent | null>(null)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const intent = resolveIntent(event)
    const data = new FormData(event.currentTarget)
    setSubmittingIntent(intent)
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
              agent_id: agentId || String(data.get("agentId") || "").trim(),
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

      await addServiceFromConfig({
        name,
        scope: scopeRef,
        config,
        lifecycle,
        handshakeMode: configFields.handshakeMode,
      })

      if (intent === "add-connect") {
        // One-shot connect for this add only — does not change startup_policy.
        await connectInstance({ service_name: name, scope: scopeRef })
        toast.success("Service added and connecting")
      } else {
        toast.success("Service added")
      }
      await onAdded()
      onBack()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Add service failed")
    } finally {
      setSubmittingIntent(null)
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
    submitting: submittingIntent !== null,
    submittingIntent,
  }
}
