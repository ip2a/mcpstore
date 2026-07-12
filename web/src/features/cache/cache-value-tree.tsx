import { useState } from "react"
import { ChevronRightIcon } from "lucide-react"

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { cn } from "@/lib/utils"

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function formatScalar(value: unknown): string {
  if (value === null) return "null"
  if (typeof value === "string") return JSON.stringify(value)
  if (typeof value === "boolean") return value ? "true" : "false"
  if (typeof value === "number") return String(value)
  return String(value)
}

function nodeLabel(name: string | number, value: unknown) {
  if (value !== null && typeof value === "object") {
    if (Array.isArray(value)) return `${name} [${value.length}]`
    return `${name} {${Object.keys(value as Record<string, unknown>).length}}`
  }
  return String(name)
}

function CacheValueTreeNode({
  name,
  value,
  depth = 0,
  defaultOpen = false,
}: {
  name?: string | number
  value: unknown
  depth?: number
  defaultOpen?: boolean
}) {
  const [open, setOpen] = useState(defaultOpen || depth < 1)
  const isArray = Array.isArray(value)
  const isObject = isRecord(value)
  const hasChildren = isArray || isObject

  if (!hasChildren) {
    return (
      <div
        className="flex min-w-0 items-start gap-2 py-0.5 font-mono text-xs"
        style={{ paddingLeft: `${depth * 14}px` }}
      >
        {name !== undefined ? (
          <>
            <span className="shrink-0 text-muted-foreground">{name}</span>
            <span className="text-muted-foreground">:</span>
          </>
        ) : null}
        <span className="min-w-0 break-all text-foreground">{formatScalar(value)}</span>
      </div>
    )
  }

  const entries = isArray
    ? value.map((item, index) => [index, item] as const)
    : Object.entries(value).sort(([a], [b]) => a.localeCompare(b))

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger
        className={cn(
          "flex w-full min-w-0 items-center gap-1 rounded-sm py-0.5 text-left font-mono text-xs hover:bg-accent/60",
        )}
        style={{ paddingLeft: `${depth * 14}px` }}
      >
        <ChevronRightIcon className={cn("size-3 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")} />
        <span className="truncate text-foreground">{name !== undefined ? nodeLabel(name, value) : isArray ? `[${value.length}]` : "{…}"}</span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="flex flex-col">
          {entries.map(([entryName, entryValue]) => (
            <CacheValueTreeNode
              key={`${String(name ?? "root")}:${String(entryName)}`}
              name={entryName}
              value={entryValue}
              depth={depth + 1}
              defaultOpen={depth < 1}
            />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

export function CacheValueTree({ value }: { value: Record<string, unknown> }) {
  const entries = Object.entries(value).sort(([a], [b]) => a.localeCompare(b))

  if (entries.length === 0) {
    return <p className="text-sm text-muted-foreground">{"{}"}</p>
  }

  return (
    <div className="flex flex-col gap-0.5">
      {entries.map(([name, entryValue]) => (
        <CacheValueTreeNode key={name} name={name} value={entryValue} defaultOpen />
      ))}
    </div>
  )
}
