import type { HTMLAttributes } from "react"

import { cn } from "@/lib/utils"

type PanelCardProps = HTMLAttributes<HTMLElement> & {
  variant?: "stack" | "plain"
}

export function PanelCard({ variant = "stack", className, ...props }: PanelCardProps) {
  return (
    <section
      className={cn(
        variant === "stack" && "flex min-h-0 flex-col gap-4 overflow-hidden rounded-lg border bg-card p-4",
        variant === "plain" && "min-h-0 overflow-hidden rounded-lg border bg-card p-4",
        className,
      )}
      {...props}
    />
  )
}
