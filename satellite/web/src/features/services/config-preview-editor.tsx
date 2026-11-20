import { useRef, type ChangeEvent } from "react"

import { highlightServiceConfig } from "@/lib/config-syntax-highlight"
import type { ServiceConfigFormat } from "@/features/services/service-config-draft"
import { cn } from "@/lib/utils"

const editorSurface =
  "m-0 box-border h-full min-h-0 w-full overflow-auto px-2.5 pb-2.5 font-mono text-sm leading-relaxed whitespace-pre-wrap break-words [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"

export function ConfigPreviewEditor({
  value,
  format,
  onChange,
  className,
}: {
  value: string
  format: ServiceConfigFormat
  onChange: (value: string) => void
  className?: string
}) {
  const highlightRef = useRef<HTMLPreElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)

  function syncScroll() {
    const input = inputRef.current
    const highlight = highlightRef.current
    if (!input || !highlight) return
    highlight.scrollTop = input.scrollTop
    highlight.scrollLeft = input.scrollLeft
  }

  function handleChange(event: ChangeEvent<HTMLTextAreaElement>) {
    onChange(event.target.value)
  }

  return (
    <div className={cn("relative min-h-0 flex-1 overflow-hidden", className)}>
      <pre
        ref={highlightRef}
        aria-hidden
        className={cn(editorSurface, "pointer-events-none absolute inset-0 text-white")}
      >
        {highlightServiceConfig(value, format)}
        {"\n"}
      </pre>
      <textarea
        ref={inputRef}
        value={value}
        spellCheck={false}
        wrap="soft"
        className={cn(
          editorSurface,
          "absolute inset-0 resize-none border-0 bg-transparent text-transparent caret-white outline-none",
        )}
        onChange={handleChange}
        onScroll={syncScroll}
      />
    </div>
  )
}
