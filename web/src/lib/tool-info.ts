import type { ToolInfo } from "@/lib/api"

export function getToolSchema(tool: ToolInfo) {
  return tool.schema || tool.inputSchema || tool.input_schema || tool.parameters || {}
}

export function getToolServiceName(tool: ToolInfo) {
  return String(tool.service_name || tool.service || tool.server_name || "") || undefined
}

export function toolKey(tool: ToolInfo) {
  return `${getToolServiceName(tool) || "tool"}:${tool.name}`
}
