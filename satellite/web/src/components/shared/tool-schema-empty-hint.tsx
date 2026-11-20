import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

export function ToolSchemaEmptyHint({
  children,
  className,
  align = "left",
}: {
  children: ReactNode
  className?: string
  align?: "left" | "right"
}) {
  return (
    <p
      className={cn(
        "text-sm leading-relaxed text-muted-foreground",
        align === "right" && "text-right",
        className,
      )}
    >
      {children}
    </p>
  )
}
