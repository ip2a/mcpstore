import { useEffect, useState } from "react"
import { PlusIcon, Trash2Icon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/lib/i18n-context"
import {
  createKeyValuePairId,
  keyValuePairsFromText,
  keyValuePairsToText,
  type KeyValuePairEntry,
} from "@/lib/key-value-pairs"
import { cn } from "@/lib/utils"

function createEmptyEntry() {
  return { id: createKeyValuePairId(), key: "", value: "" }
}

function normalizeEntries(text: string, defaultEmptyRow: boolean) {
  const parsed = keyValuePairsFromText(text)
  if (parsed.length) return parsed
  return defaultEmptyRow ? [createEmptyEntry()] : []
}

export function KeyValuePairsEditor({
  className,
  defaultEmptyRow = false,
  idPrefix,
  keyPlaceholder,
  onChange,
  value,
  valueField = "input",
  valuePlaceholder,
}: {
  className?: string
  defaultEmptyRow?: boolean
  idPrefix: string
  keyPlaceholder?: string
  onChange: (value: string) => void
  value: string
  valueField?: "input" | "textarea"
  valuePlaceholder?: string
}) {
  const { t } = useI18n()
  const [entries, setEntries] = useState<KeyValuePairEntry[]>(() => normalizeEntries(value, defaultEmptyRow))

  useEffect(() => {
    setEntries((current) => {
      if (keyValuePairsToText(current) === value) return current
      return normalizeEntries(value, defaultEmptyRow)
    })
  }, [defaultEmptyRow, value])

  function updateEntries(next: KeyValuePairEntry[]) {
    setEntries(next)
    onChange(keyValuePairsToText(next))
  }

  function addEntryAfter(id: string) {
    const index = entries.findIndex((entry) => entry.id === id)
    if (index === -1) {
      updateEntries([...entries, createEmptyEntry()])
      return
    }

    const next = [...entries]
    next.splice(index + 1, 0, createEmptyEntry())
    updateEntries(next)
  }

  function removeEntry(id: string) {
    const next = entries.filter((entry) => entry.id !== id)
    if (!next.length && defaultEmptyRow) {
      updateEntries([createEmptyEntry()])
      return
    }
    updateEntries(next)
  }

  function updateEntry(id: string, field: "key" | "value", nextValue: string) {
    updateEntries(entries.map((entry) => (entry.id === id ? { ...entry, [field]: nextValue } : entry)))
  }

  const rowLayoutClass = "flex items-start gap-2"
  const keyColumnClass = "w-[38%] shrink-0"
  const valueColumnClass = "min-w-0 flex-1"
  const actionsClass = "flex shrink-0 items-start"

  return (
    <div className={cn("flex flex-col gap-3", className)}>
      {entries.length ? (
        <div className="flex flex-col gap-2">
          <div className={cn(rowLayoutClass, "text-xs font-medium text-muted-foreground")}>
            <span className={keyColumnClass}>{t("kvKey")}</span>
            <span className={valueColumnClass}>{t("kvValue")}</span>
            <span className={cn(actionsClass, "sr-only")}>{t("add")}</span>
          </div>

          {entries.map((entry, index) => (
            <div key={entry.id} className={rowLayoutClass}>
              <Input
                id={`${idPrefix}-key-${index}`}
                className={keyColumnClass}
                value={entry.key}
                placeholder={keyPlaceholder}
                autoComplete="off"
                onChange={(event) => updateEntry(entry.id, "key", event.target.value)}
              />
              {valueField === "textarea" ? (
                <Textarea
                  id={`${idPrefix}-value-${index}`}
                  rows={2}
                  value={entry.value}
                  placeholder={valuePlaceholder}
                  autoComplete="off"
                  className={cn(valueColumnClass, "field-sizing-auto min-h-9 resize-y")}
                  onChange={(event) => updateEntry(entry.id, "value", event.target.value)}
                />
              ) : (
                <Input
                  id={`${idPrefix}-value-${index}`}
                  className={valueColumnClass}
                  value={entry.value}
                  placeholder={valuePlaceholder}
                  autoComplete="off"
                  onChange={(event) => updateEntry(entry.id, "value", event.target.value)}
                />
              )}
              <div className={actionsClass}>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("add")}
                  onClick={() => addEntryAfter(entry.id)}
                >
                  <PlusIcon />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("delete")}
                  onClick={() => removeEntry(entry.id)}
                >
                  <Trash2Icon />
                </Button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">{t("kvPairsEmpty")}</p>
      )}
    </div>
  )
}
