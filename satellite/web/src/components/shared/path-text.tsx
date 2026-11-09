import type { HTMLAttributes } from "react"

import { cn } from "@/lib/utils"

type PathTextProps = HTMLAttributes<HTMLSpanElement> & {
  value: string | null | undefined
  fallback?: string
  tone?: "default" | "muted"
  weight?: "normal" | "medium"
  wrap?: "words" | "all" | "truncate"
}

export function PathText({
  value,
  fallback = "-",
  tone = "muted",
  weight = "normal",
  wrap = "words",
  className,
  ...props
}: PathTextProps) {
  const text = value || fallback

  return (
    <span
      className={cn(
        "font-mono text-xs",
        tone === "muted" ? "text-muted-foreground" : "text-foreground",
        weight === "medium" && "font-medium",
        wrap === "words" && "break-words",
        wrap === "all" && "break-all",
        wrap === "truncate" && "truncate",
        className,
      )}
      title={value || undefined}
      {...props}
    >
      {text}
    </span>
  )
}
