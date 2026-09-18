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

export const DEFAULT_HOST_NAME = "local";
export const DEFAULT_API_BASE = "/api";

type LegacyConnection = {
  id?: string;
  url?: string;
};

export function defaultHostsConfig(): HostsConfig {
  return {
    active: DEFAULT_HOST_NAME,
    entries: {
      [DEFAULT_HOST_NAME]: { url: DEFAULT_API_BASE },
    },
  };
}

export function parseHostsPayload(data: unknown): HostsConfig {
  if (!data || typeof data !== "object") {
    return defaultHostsConfig();
  }

  const obj = data as Record<string, unknown>;
  const entries: Record<string, HostEntry> = {};
  for (const [key, value] of Object.entries(obj)) {
    if (key === "active") continue;
    if (value && typeof value === "object" && "url" in value) {
      const url = (value as HostEntry).url;
      if (typeof url === "string" && url.trim()) {
        entries[key] = { url: url.trim() };
      }
    }
  }

  if (Object.keys(entries).length === 0) {
    return defaultHostsConfig();
  }

  const activeCandidate =
    typeof obj.active === "string" && obj.active.trim()
      ? obj.active.trim()
      : DEFAULT_HOST_NAME;
  const active = entries[activeCandidate]
    ? activeCandidate
    : Object.keys(entries)[0]!;

  return { active, entries };
}

export function serializeHostsPayload(hosts: HostsConfig): HostsPayload {
  return {
    active: hosts.active,
    ...hosts.entries,
  };
}

export function resolveActiveHostUrl(hosts: HostsConfig): string {
  return (
    hosts.entries[hosts.active]?.url ??
    Object.values(hosts.entries)[0]?.url ??
    DEFAULT_API_BASE
  );
}

export function isDefaultHostsOnly(hosts: HostsConfig): boolean {
  const names = Object.keys(hosts.entries);
  return (
    names.length === 1 &&
    names[0] === DEFAULT_HOST_NAME &&
    hosts.entries[DEFAULT_HOST_NAME]?.url === DEFAULT_API_BASE
  );
}

export function migrateHostsFromLegacyStorage(): HostsConfig | null {
  try {
    const raw = localStorage.getItem("mcpstore:connections");
    if (!raw) return null;

    const parsed = JSON.parse(raw) as LegacyConnection[];
    if (!Array.isArray(parsed) || parsed.length === 0) return null;

    const entries: Record<string, HostEntry> = {};
    for (const item of parsed) {
      const url = item?.url?.trim();
      if (!url) continue;
      const name = suggestHostName(url, Object.keys(entries));
      entries[name] = { url };
    }

    if (Object.keys(entries).length === 0) return null;

    const apiBase = localStorage.getItem("mcpstore:api-base")?.trim() || DEFAULT_API_BASE;
    const active =
      Object.entries(entries).find(([, entry]) => entry.url === apiBase)?.[0] ??
      Object.keys(entries)[0]!;

    return { active, entries };
  } catch {
    return null;
  }
}

function suggestHostName(url: string, existing: string[]): string {
  if (url === DEFAULT_API_BASE) {
    return existing.includes(DEFAULT_HOST_NAME) ? nextUniqueName("local", existing) : DEFAULT_HOST_NAME;
  }

  try {
    const parsed = url.startsWith("http://") || url.startsWith("https://")
      ? new URL(url)
      : new URL(url, window.location.origin);
    const base = parsed.port ? `${parsed.hostname}:${parsed.port}` : parsed.hostname;
    return existing.includes(base) ? nextUniqueName(base, existing) : base;
  } catch {
    return nextUniqueName(url, existing);
  }
}

function nextUniqueName(base: string, existing: string[]): string {
  let index = 2;
  let candidate = base;
  while (existing.includes(candidate)) {
    candidate = `${base} ${index}`;
    index += 1;
  }
  return candidate;
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
