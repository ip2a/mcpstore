export type HostEntry = {
  url: string;
};

export type HostsConfig = {
  active: string;
  entries: Record<string, HostEntry>;
};

export type HostsPayload = {
  active: string;
  [name: string]: string | HostEntry;
};

export const DEFAULT_API_BASE = "/api";

export function parseHostsPayload(data: HostsPayload): HostsConfig {
  const entries: Record<string, HostEntry> = {};
  for (const [key, value] of Object.entries(data)) {
    if (key === "active") continue;
    if (value && typeof value === "object" && "url" in value) {
      const url = (value as HostEntry).url.trim();
      if (url) entries[key] = { url };
    }
  }

  if (Object.keys(entries).length === 0) {
    throw new Error("Hosts config has no entries");
  }

  const active = data.active.trim();
  if (!active || !entries[active]) {
    throw new Error(`Active host "${active}" is not configured`);
  }

  return { active, entries };
}

export function serializeHostsPayload(hosts: HostsConfig): HostsPayload {
  return {
    active: hosts.active,
    ...hosts.entries,
  };
}

export function resolveActiveHostUrl(hosts: HostsConfig): string {
  return hosts.entries[hosts.active].url;
}

export function validateHostsConfig(hosts: HostsConfig): string | null {
  const names = Object.keys(hosts.entries);
  if (names.length === 0) return "At least one host is required";
  if (!hosts.active.trim()) return "Active host is required";
  if (!hosts.entries[hosts.active]) return "Active host does not exist";

  for (const name of names) {
    if (!name.trim()) return "Host name cannot be empty";
    if (!hosts.entries[name]?.url.trim()) return `URL for host "${name}" cannot be empty`;
  }

  return null;
}
