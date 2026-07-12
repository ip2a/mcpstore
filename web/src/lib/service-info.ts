import type { ServiceEntry } from "@/lib/api"
import { formatKvLines } from "@/lib/api"

export function getServiceTransport(service: ServiceEntry) {
  const config = service.config as Record<string, unknown> | undefined
  return String(service.transport || config?.transport || "unknown").trim().toLowerCase()
}

export function getServiceLaunchCommand(service: ServiceEntry) {
  const config = service.config as Record<string, unknown> | undefined
  const command = String(service.command || config?.command || "").trim()
  const args = Array.isArray(config?.args) ? config.args.map(String).map((item) => item.trim()).filter(Boolean) : []
  return [command, ...args].filter(Boolean).join(" ")
}

export function formatServiceLaunchLine(service: ServiceEntry) {
  const transport = getServiceTransport(service)
  const config = service.config as Record<string, unknown> | undefined
  const launchCommand = getServiceLaunchCommand(service)
  const url = String(service.url || config?.url || "").trim()

  if (url) {
    return [transport !== "unknown" ? transport : null, url].filter(Boolean).join(" · ")
  }

  return [transport !== "unknown" ? transport : null, launchCommand || null].filter(Boolean).join(" · ") || "-"
}

export function getServiceEditValues(service: ServiceEntry) {
  const config = service.config as Record<string, unknown> | undefined
  const transport = getServiceTransport(service) as "stdio" | "streamable-http" | "sse"
  const commandOrUrl =
    transport === "stdio"
      ? getServiceLaunchCommand(service)
      : String(service.url || config?.url || "").trim()

  return {
    transport,
    commandOrUrl,
    description: String(config?.description || "").trim(),
    workingDir: String(config?.working_dir || config?.workingDir || "").trim(),
    env: formatKvLines(config?.env as Record<string, unknown> | undefined),
    headers: formatKvLines(config?.headers as Record<string, unknown> | undefined),
  }
}
