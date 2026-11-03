// Registry and market APIs for client plugins (coding-client adapters); all are app-owned endpoints.
import { appApi } from "./client";

export type ClientPlugin = {
  id: string;
  label: string;
  plugin_version: string | null;
  spec_version: number | null;
  enabled: boolean;
  /** Parse or spec failure reason; null = usable. */
  error: string | null;
  market_version: string | null;
};

export type ClientPluginList = {
  plugins: ClientPlugin[];
  last_detect: string | null;
  market_fetched_at: string | null;
};

export type MarketPlugin = {
  id: string;
  label: string;
  plugin_version: string;
  description: string | null;
  /** Installed plugin_version when the plugin is on disk, else null. */
  installed: string | null;
  update_available: boolean;
};

export type DetectHit = {
  id: string;
  label: string;
  path: string;
  by: "config_file" | "config_dir" | "command";
};

export function listClientPlugins() {
  return appApi<ClientPluginList>("/plugins");
}

export function listMarketPlugins() {
  return appApi<{ plugins: MarketPlugin[] }>("/plugins/market");
}

export function detectMarketClients() {
  return appApi<{ detected: DetectHit[] }>("/plugins/detect", { method: "POST" });
}

export function installClientPlugin(id: string) {
  return appApi(`/plugins/${id}/install`, { method: "POST" });
}

export function uninstallClientPlugin(id: string) {
  return appApi(`/plugins/${id}/uninstall`, { method: "POST" });
}

export function enableClientPlugin(id: string) {
  return appApi(`/plugins/${id}/enable`, { method: "POST" });
}

export function disableClientPlugin(id: string) {
  return appApi(`/plugins/${id}/disable`, { method: "POST" });
}
