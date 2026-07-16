import { useEffect, useState } from "react"
import { PlusIcon, Trash2Icon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useI18n } from "@/lib/i18n-context"
import {
  createStringListId,
  stringListFromText,
  stringListToText,
  type StringListEntry,
} from "@/lib/string-list"
import { cn } from "@/lib/utils"

function createEmptyEntry() {
  return { id: createStringListId(), value: "" }
}

export function StringListEditor({
  className,
  defaultEmptyRow = false,
  idPrefix,
  onChange,
  placeholder,
  value,
}: {
  className?: string
  defaultEmptyRow?: boolean
  idPrefix: string
  onChange: (value: string) => void
  placeholder?: string
  value: string
}) {
  const { t } = useI18n()
  const [entries, setEntries] = useState<StringListEntry[]>(() => stringListFromText(value, defaultEmptyRow))

  useEffect(() => {
    setEntries((current) => {
      if (stringListToText(current) === value) return current
      return stringListFromText(value, defaultEmptyRow)
    })
  }, [defaultEmptyRow, value])

  function updateEntries(next: StringListEntry[]) {
    setEntries(next)
    onChange(stringListToText(next))
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

  function updateEntry(id: string, nextValue: string) {
    updateEntries(entries.map((entry) => (entry.id === id ? { ...entry, value: nextValue } : entry)))
  }

  return (
    <div className={cn("flex flex-col gap-3", className)}>
      {entries.length ? (
        <div className="flex flex-col gap-2">
          {entries.map((entry, index) => (
            <div key={entry.id} className="flex items-start gap-2">
              <Input
                id={`${idPrefix}-${index}`}
                className="min-w-0 flex-1"
                value={entry.value}
                placeholder={placeholder}
                autoComplete="off"
                onChange={(event) => updateEntry(entry.id, event.target.value)}
              />
              <div className="flex shrink-0 items-start">
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
