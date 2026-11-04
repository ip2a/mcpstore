import { SearchIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { SearchBox } from "@/components/shared/search-box"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

type CollapsibleSearchBoxProps = {
  id?: string
  inline?: boolean
  placeholder: string
  value: string
  onChange: (value: string) => void
}

// Collapsed: icon button; expanded: search input filling the remaining space
// - Force expand when there is a value
// - Escape clears and collapses
// - Collapses on blur when empty
export function CollapsibleSearchBox({
  id,
  inline = false,
  placeholder,
  value,
  onChange,
}: CollapsibleSearchBoxProps) {
  const [open, setOpen] = useState(() => value.length > 0)
  const inputRef = useRef<HTMLInputElement>(null)

  // Auto-focus when expanded
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // External clear → keep collapsed semantics: no extra action when value becomes empty while collapsed
  // External assignment → ensure expanded
  useEffect(() => {
    if (value) setOpen(true)
  }, [value])

  if (!open) {
    return (
      <Button
        variant="outline"
        size="sm"
        className={cn("gap-1.5", !inline && "ml-auto")}
        onClick={() => setOpen(true)}
        aria-label={placeholder}
      >
        <SearchIcon size={16} />
        <span className="sr-only">{placeholder}</span>
      </Button>
    )
  }

  return (
    // ml-auto + flex-1: collapsed state hugs right, expanded state takes all remaining space
    <div className={cn("flex min-w-0 items-center", !inline && "ml-auto flex-1", inline && "w-48")}>
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
