export type HostEntry = {
  /** Candidate base URLs in preference order; the first reachable one wins. */
  urls: string[];
};

export type HostsConfig = {
  active: string;
  entries: Record<string, HostEntry>;
};

export type HostsPayload = {
  active: string;
  [name: string]: string | HostEntry;
};

export function parseHostsPayload(data: HostsPayload): HostsConfig {
  const entries: Record<string, HostEntry> = {};
  for (const [key, value] of Object.entries(data)) {
    if (key === "active") continue;
    if (value && typeof value === "object" && "urls" in value) {
      const urls = (value as HostEntry).urls
        .map((url) => url.trim())
        .filter((url) => url !== "");
      if (urls.length > 0) entries[key] = { urls };
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

/** URLs of the active host in preference order. */
export function resolveActiveHostUrls(hosts: HostsConfig): string[] {
  return hosts.entries[hosts.active].urls;
}

export function validateHostsConfig(hosts: HostsConfig): string | null {
  const names = Object.keys(hosts.entries);
  if (names.length === 0) return "At least one host is required";
  if (!hosts.active.trim()) return "Active host is required";
  if (!hosts.entries[hosts.active]) return "Active host does not exist";

  for (const name of names) {
    const entry = hosts.entries[name];
    if (!name.trim()) return "Host name cannot be empty";
    if (!entry || entry.urls.length === 0) {
      return `Host "${name}" needs at least one URL`;
    }
    if (entry.urls.some((url) => !url.trim())) {
      return `Host "${name}" has an empty URL`;
    }
  }

  return null;
}
