// Registry and market APIs for client plugins (coding-client adapters); all are app-owned endpoints.
import { appApi, buildQuery } from "./client"

export type ClientPlugin = {
  id: string
  label: string
  plugin_version: string | null
  spec_version: number | null
  enabled: boolean
  /** Parse or spec failure reason; null = usable. */
  error: string | null
  market_version: string | null
}

export type ClientPluginList = {
  plugins: ClientPlugin[]
  last_detect: string | null
  market_fetched_at: string | null
}

export type MarketPlugin = {
  id: string
  label: string
  plugin_version: string
  description: string | null
  /** Installed plugin_version when the plugin is on disk, else null. */
  installed: string | null
  update_available: boolean
}

export type DetectHit = {
  id: string
  label: string
  path: string
  by: "config_file" | "config_dir" | "command"
}

export function listClientPlugins() {
  return appApi<ClientPluginList>("/plugins")
}

export function listMarketPlugins() {
  return appApi<{ plugins: MarketPlugin[] }>("/plugins/market")
}

export function detectMarketClients() {
  return appApi<{ detected: DetectHit[] }>("/plugins/detect", { method: "POST" })
}

export function installClientPlugin(id: string) {
  return appApi(`/plugins/${id}/install`, { method: "POST" })
}

export function uninstallClientPlugin(id: string) {
  return appApi(`/plugins/${id}/uninstall`, { method: "POST" })
}

export function enableClientPlugin(id: string) {
  return appApi(`/plugins/${id}/enable`, { method: "POST" })
}

export function disableClientPlugin(id: string) {
  return appApi(`/plugins/${id}/disable`, { method: "POST" })
}

// ===== MCP services market: a curated featured catalogue, then the official
// MCP Registry for search and endless scroll. Install goes through the
// regular POST /services/:name with the item's config. =====

/** One value a service needs from the user before it can start. */
export type MarketServiceInput = {
  key: string
  /** `env`, `header` or `arg`. */
  where: "env" | "header" | "arg"
  label: string
  description?: string
  placeholder?: string
  secret?: boolean
  required?: boolean
  /** A header value's template around the input, `"Bearer {}"`. */
  format?: string
}

/** A service the market offers, installable with its config payload. */
export type MarketServiceItem = {
  id: string
  name: string
  title: string
  publisher?: string
  description?: string
  icon?: string
  homepage?: string
  transport: string
  sign_in?: boolean
  matched?: boolean
  config: {
    url?: string
    command?: string
    args?: string[]
    headers?: Record<string, string>
    env?: Record<string, string>
    transport: string
  }
  inputs: MarketServiceInput[]
}

export function listFeaturedMarketServices() {
  return appApi<{ items: MarketServiceItem[] }>("/plugins/market/services")
}

export function listRegistryMarketServices(input: { q: string; offset: number; limit?: number }) {
  const query = buildQuery({ q: input.q, offset: input.offset, limit: input.limit })
  return appApi<{
    items: MarketServiceItem[]
    offset: number
    total: number
    /** Why the registry couldn't be searched; the featured list is unaffected. */
    failed?: string
  }>(`/plugins/market/services/registry${query}`)
}

/** The item's config with the user's input values filled in, ready to POST. */
export function fillMarketServiceConfig(
  item: Pick<MarketServiceItem, "config" | "inputs">,
  values: Record<string, string>,
) {
  const config: Record<string, unknown> = JSON.parse(JSON.stringify(item.config))
  for (const input of item.inputs) {
    const value = (values[input.key] ?? "").trim()
    if (!value) continue
    if (input.where === "env") {
      config.env = { ...(config.env as Record<string, string> | undefined), [input.key]: value }
    } else if (input.where === "header") {
      const format = input.format || "{}"
      config.headers = {
        ...(config.headers as Record<string, string> | undefined),
        [input.key]: format.replace("{}", value),
      }
    } else if (input.where === "arg") {
      const args = (config.args as string[] | undefined) ?? []
      config.args = [...args, value]
    }
  }
  return config
}
