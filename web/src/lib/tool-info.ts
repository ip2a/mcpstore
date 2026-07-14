import type { ToolInfo } from "@/lib/api"

export function getToolSchema(tool: ToolInfo) {
  return tool.input_schema || {}
}

export function getToolOutputSchema(tool: ToolInfo) {
  return tool.output_schema || {}
}

export function getToolMeta(tool: ToolInfo) {
  return tool.meta ?? null
}

export function getToolAnnotations(tool: ToolInfo) {
  return tool.annotations ?? null
}

export function hasJsonContent(value: unknown) {
  if (value === null || value === undefined) return false
  if (Array.isArray(value)) return value.length > 0
  if (typeof value === "object") return Object.keys(value).length > 0
  return true
}

export function parseToolDescription(description: string) {
  const trimmed = description.trim()
  if (!trimmed) return { summary: "", sections: [] as Array<{ label: string; content: string }> }

  const parts = trimmed.split(/\n\n+/)
  const summary = parts[0] || ""
  const sections: Array<{ label: string; content: string }> = []

  for (const part of parts.slice(1)) {
    const match = part.match(/^(Args|Returns|Parameters|Return|Raises):\s*/i)
    if (match) {
      sections.push({ label: match[1], content: part.slice(match[0].length).trim() })
      continue
    }
    sections.push({ label: "", content: part.trim() })
  }

  return { summary, sections }
}

export function toolKey(instanceId: string, tool: ToolInfo) {
  return `${instanceId}:${tool.name}`
}

export function findToolStatus(toolName: string, statusReport?: { tools?: Array<{ tool_name: string; status: string }> } | null) {
  return statusReport?.tools?.find((item) => item.tool_name === toolName)
}

type SchemaField = Record<string, unknown>

export function resolveSchemaFieldType(field: SchemaField): string {
  if (typeof field.type === "string") return field.type
  if (Array.isArray(field.type)) return field.type.join(" | ")

  const anyOf = field.anyOf as SchemaField[] | undefined
  if (anyOf?.length) {
    const types = anyOf
      .map((item) => {
        if (item.type === "null") return "null"
        if (typeof item.type === "string") return item.type
        if (item.type === "array") {
          const itemType = (item.items as SchemaField | undefined)?.type
          return typeof itemType === "string" ? `${itemType}[]` : "array"
        }
        return null
      })
      .filter(Boolean)
    if (types.length) return types.join(" | ")
  }

  if (field.type === "array") {
    const itemType = (field.items as SchemaField | undefined)?.type
    if (typeof itemType === "string") return `${itemType}[]`
    return "array"
  }

  return "any"
}

export function formatSchemaDefaultValue(value: unknown): string {
  if (value === null) return "null"
  if (typeof value === "string") return JSON.stringify(value)
  if (typeof value === "boolean" || typeof value === "number") return String(value)
  return JSON.stringify(value)
}

export function getSchemaFieldSubtitle(field: SchemaField): string | null {
  const title = typeof field.title === "string" ? field.title.trim() : ""
  if (title) return title

  const description = typeof field.description === "string" ? field.description.trim() : ""
  return description || null
}
