import { useState, type FormEvent } from "react"
import { toast } from "sonner"

import { addService, parseKvLines } from "@/lib/api"

export type AddServiceScope = "store" | "agent"
export type AddServiceTransport = "stdio" | "streamable-http" | "sse"

export function useAddServiceForm({ onAdded, onBack }: { onAdded: () => Promise<void>; onBack: () => void }) {
  const [scope, setScope] = useState<AddServiceScope>("store")
  const [transport, setTransport] = useState<AddServiceTransport>("stdio")
  const [agentId, setAgentId] = useState("")
  const [submitting, setSubmitting] = useState(false)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    setSubmitting(true)
    try {
      await addService({
        name: String(data.get("name") || "").trim(),
        scope,
        agentId: scope === "agent" ? agentId || String(data.get("agentId") || "").trim() : undefined,
        transport,
        commandOrUrl: String(data.get("commandOrUrl") || "").trim(),
        description: String(data.get("description") || "").trim() || undefined,
        workingDir: String(data.get("workingDir") || "").trim() || undefined,
        env: parseKvLines(String(data.get("env") || "")),
        headers: parseKvLines(String(data.get("headers") || "")),
      })
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
    onSubmit,
    scope,
    setAgentId,
    setScope,
    setTransport,
    submitting,
    transport,
  }
}
