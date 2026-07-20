import type { ServiceInstance } from "@/lib/api"
import { formatKvLines } from "@/lib/api"

export function getServiceTransport(service: ServiceInstance) {
  return service.transport.trim().toLowerCase()
}

export function getServiceLaunchCommand(service: ServiceInstance) {
  const config = service.effective_config
  const command = String(service.command || "").trim()
  const args = Array.isArray(config?.args) ? config.args.map(String).map((item) => item.trim()).filter(Boolean) : []
  return [command, ...args].filter(Boolean).join(" ")
}

export function formatServiceLaunchLine(service: ServiceInstance) {
  const transport = getServiceTransport(service)
  const config = service.effective_config
  const launchCommand = getServiceLaunchCommand(service)
  const url = String(service.url || "").trim()

  if (url) {
    return [transport !== "unknown" ? transport : null, url].filter(Boolean).join(" · ")
  }

  return [transport !== "unknown" ? transport : null, launchCommand || null].filter(Boolean).join(" · ") || "-"
}

export function getServiceEditValues(service: ServiceInstance) {
  const config = service.effective_config
  const transport = getServiceTransport(service) as "stdio" | "streamable-http"
  const commandOrUrl =
    transport === "stdio"
      ? getServiceLaunchCommand(service)
      : String(service.url || "").trim()

  return {
    transport,
    commandOrUrl,
    description: String(config?.description || "").trim(),
    workingDir: String(config.workingDir || "").trim(),
    env: formatKvLines(config?.env as Record<string, unknown> | undefined),
    headers: formatKvLines(config?.headers as Record<string, unknown> | undefined),
  }
}
