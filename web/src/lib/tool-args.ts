import { getSchemaFieldSubtitle, resolveSchemaFieldType } from "@/lib/tool-info"

import { formatSchemaDefaultValue } from "@/lib/tool-info"

type SchemaField = Record<string, unknown>
export type ToolSchema = { properties?: Record<string, SchemaField>; required?: string[] }

export type FormFieldType = "boolean" | "integer" | "number" | "string" | "enum" | "json"

export function resolveFormFieldType(field: SchemaField): FormFieldType {
  if (Array.isArray(field.enum) && field.enum.length > 0) return "enum"

  const resolved = resolveSchemaFieldType(field)
  if (resolved === "boolean") return "boolean"
  if (resolved === "integer") return "integer"
  if (resolved === "number") return "number"
  if (resolved === "string" || resolved.includes("string")) return "string"
  return "json"
}

export function getSchemaDefaultValue(field: SchemaField): unknown {
  if ("default" in field) return field.default

  const type = resolveFormFieldType(field)
  if (type === "boolean") return false
  if (type === "integer" || type === "number") return ""
  if (type === "string" || type === "enum") return ""
  return null
}

export function buildDefaultArgs(schema: ToolSchema): Record<string, unknown> {
  const properties = schema.properties || {}
  return Object.fromEntries(
    Object.entries(properties).map(([name, field]) => [name, getSchemaDefaultValue(field)]),
  )
}

export function serializeToolArgs(values: Record<string, unknown>, schema: ToolSchema): Record<string, unknown> {
  const properties = schema.properties || {}
  const result: Record<string, unknown> = {}

  for (const [name, field] of Object.entries(properties)) {
    const value = values[name]
    const type = resolveFormFieldType(field)

    if (type === "boolean") {
      result[name] = Boolean(value)
      continue
    }

    if (type === "integer") {
      if (value === "" || value === null || value === undefined) {
        result[name] = null
        continue
      }
      result[name] = typeof value === "number" ? value : Number.parseInt(String(value), 10)
      continue
    }

    if (type === "number") {
      if (value === "" || value === null || value === undefined) {
        result[name] = null
        continue
      }
      result[name] = typeof value === "number" ? value : Number.parseFloat(String(value))
      continue
    }

    if (type === "string" || type === "enum") {
      if (value === "" || value === null || value === undefined) {
        result[name] = null
        continue
      }
      result[name] = String(value)
      continue
    }

    if (value === "" || value === undefined) {
      result[name] = null
      continue
    }

    if (typeof value === "string") {
      try {
        result[name] = JSON.parse(value)
      } catch {
        result[name] = value
      }
      continue
    }

    result[name] = value
  }

  return result
}

export function getSortedSchemaFields(schema: ToolSchema): Array<[string, SchemaField]> {
  return Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))
}

export function getSchemaFieldLabel(name: string, field: SchemaField): string {
  const subtitle = getSchemaFieldSubtitle(field)
  if (subtitle) return subtitle
  return name
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ")
}

export function isSchemaFieldRequired(name: string, schema: ToolSchema): boolean {
  return schema.required?.includes(name) ?? false
}

export function formatDefaultPlaceholder(field: SchemaField): string {
  if (!("default" in field)) {
    const type = resolveFormFieldType(field)
    if (type === "boolean") return "false"
    if (type === "integer" || type === "number") return "null"
    return "null"
  }
  return formatSchemaDefaultValue(field.default)
}

export function isEmptyFormValue(value: unknown): boolean {
  return value === "" || value === null || value === undefined
}

export function isFullWidthField(field: SchemaField): boolean {
  return resolveFormFieldType(field) === "json"
}
