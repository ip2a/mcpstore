import { useState, type FormEvent } from "react"
import { parse as parseToml } from "smol-toml"
import { toast } from "sonner"

import { addService, addServiceFromConfig, parseKvLines, type ServiceRestartPolicy, type ServiceStartupPolicy } from "@/lib/api"
import { buildServiceLifecycleConfig } from "@/features/services/service-lifecycle"

export type AddServiceScope = "store" | "agent"
export type AddServiceTransport = "stdio" | "streamable-http" | "sse"
export type AddServiceMode = AddServiceTransport | "json" | "toml"

const JSON_PLACEHOLDER = `{
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
  "transport": "stdio"
}`

const TOML_PLACEHOLDER = `command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
transport = "stdio"`

export function getConfigTextPlaceholder(mode: "json" | "toml") {
  return mode === "json" ? JSON_PLACEHOLDER : TOML_PLACEHOLDER
}

function parseConfigText(mode: "json" | "toml", text: string): Record<string, unknown> {
  if (!text.trim()) {
    throw new Error(mode === "json" ? "JSON config is required" : "TOML config is required")
  }
  if (mode === "json") {
    const parsed = JSON.parse(text) as unknown
    if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
      throw new Error("JSON config must be an object")
    }
    return parsed as Record<string, unknown>
  }
  const parsed = parseToml(text)
  if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
    throw new Error("TOML config must be a table/object")
  }
  return parsed as Record<string, unknown>
}

export function useAddServiceForm({ onAdded, onBack }: { onAdded: () => Promise<void>; onBack: () => void }) {
  const [scope, setScope] = useState<AddServiceScope>("store")
  const [mode, setMode] = useState<AddServiceMode>("stdio")
  const [agentId, setAgentId] = useState("")
  const [startupPolicy, setStartupPolicy] = useState<ServiceStartupPolicy>("lazy")
  const [restartPolicy, setRestartPolicy] = useState<ServiceRestartPolicy>("no")
  const [submitting, setSubmitting] = useState(false)

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    setSubmitting(true)
    try {
      const name = String(data.get("name") || "").trim()
      const scopeRef =
        scope === "store"
          ? ({ type: "store" } as const)
          : ({ type: "agent", agent_id: agentId || String(data.get("agentId") || "").trim() } as const)
      const lifecycle = buildServiceLifecycleConfig({ startupPolicy, restartPolicy })

      if (mode === "json" || mode === "toml") {
        const config = parseConfigText(mode, String(data.get("configText") || ""))
        await addServiceFromConfig({ name, scope: scopeRef, config, lifecycle })
      } else {
        await addService({
          name,
          scope: scopeRef,
          transport: mode,
          commandOrUrl: String(data.get("commandOrUrl") || "").trim(),
          description: String(data.get("description") || "").trim() || undefined,
          workingDir: String(data.get("workingDir") || "").trim() || undefined,
          env: parseKvLines(String(data.get("env") || "")),
          headers: parseKvLines(String(data.get("headers") || "")),
          lifecycle,
        })
      }
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
    mode,
    onSubmit,
    restartPolicy,
    scope,
    setAgentId,
    setMode,
    setRestartPolicy,
    setScope,
    setStartupPolicy,
    startupPolicy,
    submitting,
  }
}
