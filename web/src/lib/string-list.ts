export type StringListEntry = {
  id: string
  value: string
}

export function createStringListId() {
  return crypto.randomUUID()
}

export function stringListFromText(text: string, defaultEmptyRow: boolean): StringListEntry[] {
  const lines = text.split("\n")
  const entries = lines.map((line) => ({ id: createStringListId(), value: line }))

  if (entries.length) return entries
  return defaultEmptyRow ? [{ id: createStringListId(), value: "" }] : []
}

export function stringListToText(entries: StringListEntry[]) {
  return entries.map((entry) => entry.value).join("\n")
}

export function stringListToValues(entries: StringListEntry[]) {
  return entries.map((entry) => entry.value.trim()).filter(Boolean)
}
