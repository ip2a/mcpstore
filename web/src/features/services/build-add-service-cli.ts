import { parseKvLines } from "@/lib/api"
import type { ServiceConfigFields } from "@/features/services/service-config-draft"
import type { AddServiceScope } from "@/features/services/use-add-service-form"

function shellQuote(value: string) {
  return `'${value.replace(/'/g, `'\\''`)}'`
}

export function buildAddServiceCliCommand(input: {
  agentId?: string
  fields: ServiceConfigFields
  name: string
  scope: AddServiceScope
}): string {
  const name = input.name.trim() || "github"
  const scopeFlag =
    input.scope === "agent" && input.agentId?.trim()
      ? ` \\\n  --for-agent ${shellQuote(input.agentId.trim())}`
      : ""

  if (input.fields.transport === "stdio") {
    const env = parseKvLines(input.fields.envText)
    const envFlags = Object.entries(env)
      .map(([key, value]) => ` \\\n  --env ${key}=${shellQuote(value)}`)
      .join("")
    const command = input.fields.command.trim() || "npx"
    const args = input.fields.argsText
      .split("\n")
      .map((item) => item.trim())
      .filter(Boolean)
    const cmdLine = [command, ...args].join(" ")

    return [
      "mcpstore add \\",
      "  --transport stdio \\",
      `  ${name}${scopeFlag}${envFlags} \\`,
      `  -- ${cmdLine}`,
    ].join("\n")
  }

  const transport = input.fields.transport === "sse" ? "sse" : "http"
  const headers = parseKvLines(input.fields.headersText)
  const headerFlags = Object.entries(headers)
    .map(([key, value]) => ` \\\n  -e ${key}=${shellQuote(value)}`)
    .join("")
  const url = input.fields.url.trim() || "https://example.com/mcp"

  return [
    "mcpstore add \\",
    `  --transport ${transport} \\`,
    `  ${name} \\`,
    `  ${url}${scopeFlag}${headerFlags}`,
  ].join("\n")
}
