export function workspaceName(path: string | null | undefined, fallback = "No workspace") {
  if (!path) return fallback
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts.at(-1) || path
}
