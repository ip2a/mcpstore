import type { PromptInfo } from "@/lib/api"

export type PromptArgDef = {
  name: string
  description?: string
  required: boolean
}

export function parsePromptArgs(prompt: PromptInfo): PromptArgDef[] {
  const args = prompt.arguments
  if (!args) return []

  if (Array.isArray(args)) {
    return args
      .map((item): PromptArgDef | null => {
        if (!item || typeof item !== "object") return null
        const record = item as Record<string, unknown>
        const name = String(record.name || "").trim()
        if (!name) return null
        return {
          name,
          description:
            record.description != null ? String(record.description).trim() || undefined : undefined,
          required: Boolean(record.required),
        }
      })
      .filter((item): item is PromptArgDef => item != null)
  }

  if (typeof args === "object") {
    const schema = args as { properties?: Record<string, unknown>; required?: string[] }
    const properties = schema.properties
    if (!properties || typeof properties !== "object") return []
    const required = new Set(schema.required || [])
    return Object.entries(properties)
      .map(([name, field]) => {
        const description =
          field && typeof field === "object" && "description" in field
            ? String((field as { description?: unknown }).description || "").trim() || undefined
            : undefined
        return {
          name,
          description,
          required: required.has(name),
        } satisfies PromptArgDef
      })
      .sort((a, b) => a.name.localeCompare(b.name))
  }

  return []
}

export function buildDefaultPromptArgs(defs: PromptArgDef[]): Record<string, unknown> {
  return Object.fromEntries(defs.map((def) => [def.name, ""]))
}
