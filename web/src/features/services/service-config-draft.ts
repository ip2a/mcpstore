import { parse as parseToml, stringify as stringifyToml } from "smol-toml"

import { parseKvLines } from "@/lib/api"

export type ServiceConfigTransport = "stdio" | "streamable-http"
export type ServiceConfigFormat = "json" | "toml"

export type ServiceConfigFields = {
  transport: ServiceConfigTransport
  command: string
  argsText: string
  url: string
  description: string
  workingDir: string
  envText: string
  headersText: string
}

export const DEFAULT_SERVICE_CONFIG_FIELDS: ServiceConfigFields = {
  transport: "stdio",
  command: "npx",
  argsText: "-y\n@modelcontextprotocol/server-filesystem\n.",
  url: "",
  description: "",
  workingDir: "",
  envText: "",
  headersText: "",
}

const TRANSPORTS = new Set<ServiceConfigTransport>(["stdio", "streamable-http"])

export function getUiTransportMode(transport: ServiceConfigTransport): "stdio" | "http" {
  return transport === "stdio" ? "stdio" : "http"
}

export function resolveHttpTransport(
  mode: "stdio" | "http",
  _current: ServiceConfigTransport,
): ServiceConfigTransport {
  if (mode === "stdio") return "stdio"
  return "streamable-http"
}

function normalizeTransport(value: unknown): ServiceConfigTransport {
  return typeof value === "string" && TRANSPORTS.has(value as ServiceConfigTransport)
    ? (value as ServiceConfigTransport)
    : "stdio"
}

function formatKvText(value?: Record<string, unknown>) {
  if (!value) return ""
  return Object.entries(value)
    .map(([key, item]) => `${key}=${String(item ?? "")}`)
    .join("\n")
}

function formatArgsText(value: unknown) {
  if (!Array.isArray(value)) return ""
  return value.map((item) => String(item ?? "")).join("\n")
}

export function fieldsToConfig(fields: ServiceConfigFields): Record<string, unknown> {
  const config: Record<string, unknown> = {
    transport: fields.transport,
  }

  if (fields.description.trim()) config.description = fields.description.trim()

  if (fields.transport === "stdio") {
    const env = parseKvLines(fields.envText)
    if (fields.command.trim()) config.command = fields.command.trim()
    const args = fields.argsText
      .split("\n")
      .map((item) => item.trim())
      .filter(Boolean)
    if (args.length) config.args = args
    if (fields.workingDir.trim()) config.workingDir = fields.workingDir.trim()
    if (Object.keys(env).length) config.env = env
    return config
  }

  const headers = parseKvLines(fields.headersText)
  if (fields.url.trim()) config.url = fields.url.trim()
  if (Object.keys(headers).length) config.headers = headers
  return config
}

export function configToFields(config: Record<string, unknown>): ServiceConfigFields {
  return {
    transport: normalizeTransport(config.transport),
    command: String(config.command || ""),
    argsText: formatArgsText(config.args),
    url: String(config.url || ""),
    description: String(config.description || ""),
    workingDir: String(config.workingDir || ""),
    envText: formatKvText(config.env as Record<string, unknown> | undefined),
    headersText: formatKvText(config.headers as Record<string, unknown> | undefined),
  }
}

export type ParsedServiceConfigImport = {
  name?: string
  config: Record<string, unknown>
}

function unwrapMcpServers(parsed: Record<string, unknown>): ParsedServiceConfigImport {
  const mcpServers = parsed.mcpServers
  if (!mcpServers || typeof mcpServers !== "object" || Array.isArray(mcpServers)) {
    return { config: parsed }
  }

  const entries = Object.entries(mcpServers as Record<string, unknown>)
  if (entries.length === 0) {
    throw new Error("mcpServers must contain at least one server")
  }
  if (entries.length > 1) {
    throw new Error("Multiple servers in mcpServers; paste one server at a time")
  }

  const [name, config] = entries[0]
  if (!config || typeof config !== "object" || Array.isArray(config)) {
    throw new Error("Server config must be an object")
  }
  return { name, config: config as Record<string, unknown> }
}

export function parseConfigText(format: ServiceConfigFormat, text: string): Record<string, unknown> {
  return parseImportedServiceConfig(format, text).config
}

export function parseImportedServiceConfig(
  format: ServiceConfigFormat,
  text: string,
): ParsedServiceConfigImport {
  if (!text.trim()) {
    throw new Error(format === "json" ? "JSON config is required" : "TOML config is required")
  }
  if (format === "json") {
    const parsed = JSON.parse(text) as unknown
    if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
      throw new Error("JSON config must be an object")
    }
    return unwrapMcpServers(parsed as Record<string, unknown>)
  }
  const parsed = parseToml(text)
  if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
    throw new Error("TOML config must be a table/object")
  }
  return unwrapMcpServers(parsed as Record<string, unknown>)
}

export function serializeConfigFields(fields: ServiceConfigFields, format: ServiceConfigFormat): string {
  const config = fieldsToConfig(fields)
  if (format === "json") {
    return `${JSON.stringify(config, null, 2)}\n`
  }
  return `${stringifyToml(config)}\n`
}
