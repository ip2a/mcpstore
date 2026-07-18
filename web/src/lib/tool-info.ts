import type { ServiceState, ToolInfo } from "@/lib/api"

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

const STRUCTURED_DESCRIPTION_SECTIONS = new Set(["args", "parameters", "returns", "return", "raises"])

export function parseDescriptionParamDocs(content: string): Record<string, string> {
  const docs: Record<string, string> = {}

  for (const line of content.split("\n")) {
    const trimmed = line.trim().replace(/^[-*]\s+/, "")
    if (!trimmed) continue

    const match = trimmed.match(/^([A-Za-z_][\w.-]*)\s*:\s*(.+)$/)
    if (match) {
      docs[match[1]] = match[2].trim()
    }
  }

  return docs
}

export function extractToolDescriptionDocs(description: string) {
  const { summary, sections } = parseToolDescription(description)
  const proseInputParams: Record<string, string> = {}
  const proseOutputParams: Record<string, string> = {}
  let proseReturnSummary = ""
  let proseRaisesSummary = ""
  const otherSections: Array<{ label: string; content: string }> = []

  for (const section of sections) {
    const label = section.label.toLowerCase()

    if (label === "args" || label === "parameters") {
      Object.assign(proseInputParams, parseDescriptionParamDocs(section.content))
      continue
    }

    if (label === "returns" || label === "return") {
      const parsed = parseDescriptionParamDocs(section.content)
      if (Object.keys(parsed).length > 0) {
        Object.assign(proseOutputParams, parsed)
      } else {
        proseReturnSummary = section.content
      }
      continue
    }

    if (label === "raises") {
      proseRaisesSummary = section.content
      continue
    }

    otherSections.push(section)
  }

  return {
    summary,
    proseInputParams,
    proseOutputParams,
    proseReturnSummary,
    proseRaisesSummary,
    otherSections,
  }
}

export function isStructuredDescriptionSection(label: string) {
  return STRUCTURED_DESCRIPTION_SECTIONS.has(label.toLowerCase())
}

export function toolKey(instanceId: string, tool: ToolInfo) {
  return `${instanceId}:${tool.name}`
}

export function findToolStatus(toolName: string, state?: ServiceState | null) {
  return state?.tools.items.find((item) => item.name === toolName)
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

export function getSchemaFieldDescription(field: SchemaField): string | null {
  const description = typeof field.description === "string" ? field.description.trim() : ""
  return description || null
}

export function getSchemaFieldTitle(field: SchemaField): string | null {
  const title = typeof field.title === "string" ? field.title.trim() : ""
  return title || null
}

/** Short label for compact UI: title first, then schema description. */
export function getSchemaFieldSubtitle(field: SchemaField): string | null {
  return getSchemaFieldTitle(field) || getSchemaFieldDescription(field)
}

/**
 * MCP 工具参数说明有两个常见来源，按参数各自独立取值：
 * 1. input_schema.properties[name].description — JSON Schema 结构化说明
 * 2. tool.description 里 Args/Parameters 段的 `name: text` — docstring 风格说明
 *
 * 对单个参数：有 schema 说明就用 schema；没有再看 prose。不是“降级”，而是服务端可能只写其中一处。
 */
export function resolveParameterDoc(
  name: string,
  field: SchemaField,
  proseParamDocs: Record<string, string>,
): string | null {
  const schemaDoc = getSchemaFieldDescription(field)
  if (schemaDoc) return schemaDoc

  const proseDoc = proseParamDocs[name]?.trim()
  return proseDoc || null
}
