import type { ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

type MetaLineProps = {
  label: ReactNode
  value: ReactNode
  destructive?: boolean
  className?: string
  labelClassName?: string
  valueClassName?: string
  columns?: "default" | "wide"
}

export function MetaLine({
  label,
  value,
  destructive = false,
  className,
  labelClassName,
  valueClassName,
  columns = "default",
}: MetaLineProps) {
  const content = value === null || value === undefined || value === "" ? "-" : value

  return (
    <div className={cn("grid gap-2", columns === "wide" ? "md:grid-cols-[10rem_1fr]" : "grid-cols-[7rem_minmax(0,1fr)]", className)}>
      <span className={cn("text-muted-foreground", labelClassName)}>{label}</span>
      {destructive ? (
        <Badge variant="destructive" className={cn("w-fit max-w-full break-words", valueClassName)}>
          {content}
        </Badge>
      ) : (
        <span className={cn("break-all", valueClassName)}>{content}</span>
      )}
    </div>
  )
}
