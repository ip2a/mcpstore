import { SearchIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { SearchBox } from "@/components/shared/search-box"
import { Button } from "@/components/ui/button"

type CollapsibleSearchBoxProps = {
  id?: string
  placeholder: string
  value: string
  onChange: (value: string) => void
}

// Collapsed: an icon button; expanded: a search input filling the remaining space
// - Force expanded while a value is present
// - Escape clears and collapses
// - Blurring with no value collapses
export function CollapsibleSearchBox({ id, placeholder, value, onChange }: CollapsibleSearchBoxProps) {
  const [open, setOpen] = useState(() => value.length > 0)
  const inputRef = useRef<HTMLInputElement>(null)

  // Auto-focus when expanded
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // External clear → keep collapsed semantics: no extra action when value empties while collapsed
  // External assignment → make sure it expands
  useEffect(() => {
    if (value) setOpen(true)
  }, [value])

  if (!open) {
    return (
      <Button
        variant="outline"
        size="sm"
        className="ml-auto gap-1.5"
        onClick={() => setOpen(true)}
        aria-label={placeholder}
      >
        <SearchIcon size={16} />
        <span className="sr-only">{placeholder}</span>
      </Button>
    )
  }

  return (
    // ml-auto + flex-1: right-aligned when collapsed, consumes all remaining space when expanded
    <div className="ml-auto flex min-w-0 flex-1 items-center">
      <SearchBox
        ref={inputRef}
        id={id}
        placeholder={placeholder}
        value={value}
        onChange={onChange}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            onChange("")
            setOpen(false)
          }
        }}
        onBlur={() => {
          if (!value) setOpen(false)
        }}
      />
    </div>
  )
}