export type KeyValuePairEntry = {
  id: string
  key: string
  value: string
}

export function createKeyValuePairId() {
  return crypto.randomUUID()
}

export function keyValuePairsFromText(text: string): KeyValuePairEntry[] {
  if (!text.trim()) return []

  const entries: KeyValuePairEntry[] = []

  for (const line of text.split("\n")) {
    const trimmed = line.trim()
    if (!trimmed) continue

    const index = trimmed.indexOf("=")
    if (index <= 0) {
      entries.push({ id: createKeyValuePairId(), key: trimmed, value: "" })
      continue
    }

    entries.push({
      id: createKeyValuePairId(),
      key: trimmed.slice(0, index).trim(),
      value: trimmed.slice(index + 1),
    })
  }

  return entries
}

export function keyValuePairsToText(entries: KeyValuePairEntry[]) {
  return entries
    .filter((entry) => entry.key.trim() || entry.value.trim())
    .map((entry) => `${entry.key}=${entry.value}`)
    .join("\n")
}
