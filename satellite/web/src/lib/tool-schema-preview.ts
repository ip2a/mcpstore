import { serializeToolArgs, type ToolSchema } from "@/lib/tool-args"
import { resolveSchemaFieldType } from "@/lib/tool-info"

type SchemaField = Record<string, unknown>
type JsonSchema = { properties?: Record<string, SchemaField>; required?: string[] }

function exampleFromField(field: SchemaField): unknown {
  if ("default" in field) return field.default
  if (Array.isArray(field.enum) && field.enum.length > 0) return field.enum[0]

  const type = resolveSchemaFieldType(field)
  if (type === "string") return "string"
  if (type === "integer") return 0
  if (type === "number") return 0
  if (type === "boolean") return false
  if (type.includes("null")) return null
  if (type.includes("array")) return []
  if (type === "object") return {}

  const anyOf = field.anyOf as SchemaField[] | undefined
  if (anyOf?.length) {
    const first = anyOf.find((item) => item.type !== "null") || anyOf[0]
    return exampleFromField(first)
  }

  return null
}

export function buildSchemaExampleValue(schema: JsonSchema): unknown {
  const properties = schema.properties || {}
  const entries = Object.entries(properties)

  if (!entries.length) {
    return {
      content: [{ type: "text", text: "..." }],
      isError: false,
    }
  }

  return Object.fromEntries(entries.map(([name, field]) => [name, exampleFromField(field)]))
}

export function buildToolCliCommand(input: {
  instanceId: string
  toolName: string
  args: Record<string, unknown>
  toolArgsSchema: ToolSchema
}): string {
  const serialized = serializeToolArgs(input.args, input.toolArgsSchema)
  const argsJson = JSON.stringify(serialized)

  return [
    "mcpstore tools call \\",
    `  --instance "${input.instanceId}" \\`,
    `  --tool "${input.toolName}" \\`,
    `  --args '${argsJson}'`,
  ].join("\n")
}
