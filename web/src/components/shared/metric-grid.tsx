import type { HTMLAttributes, ReactNode } from "react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

type MetricGridProps = HTMLAttributes<HTMLDivElement> & {
  columns?: "auto" | "two" | "three" | "four"
}

type MetricTileProps = {
  active?: boolean
  className?: string
  hint?: ReactNode
  label: ReactNode
  onClick?: () => void
  title?: string
  value: ReactNode
  variant?: "bordered" | "compact" | "plain" | "square"
}

const gridColumns = {
  auto: "grid min-w-0 grid-cols-1 gap-2 @min-[28rem]:grid-cols-2 @min-[42rem]:grid-cols-4 @min-[28rem]:gap-3",
  two: "grid grid-cols-2 gap-3",
  three: "grid grid-cols-3 gap-2",
  four: "grid min-w-0 grid-cols-1 gap-2 @min-[28rem]:grid-cols-2 @min-[42rem]:grid-cols-4 @min-[28rem]:gap-3 [&>*]:min-w-0 [&>*]:overflow-hidden",
}

export function MetricGrid({ columns = "auto", className, ...props }: MetricGridProps) {
  return <div className={cn(gridColumns[columns], className)} {...props} />
}

const tileText = "min-w-0 max-w-full truncate"

export function MetricTile({
  active = false,
  className,
  hint,
  label,
  onClick,
  title,
  value,
  variant = "plain",
}: MetricTileProps) {
  if (variant === "square") {
    return (
      <div
        className={cn(
          "flex aspect-square size-[4.75rem] flex-col items-center justify-center gap-0.5 overflow-hidden rounded-md border px-1.5 py-1.5 text-center sm:size-[5rem]",
          className,
        )}
        title={title ?? (typeof label === "string" ? label : undefined)}
      >
        <span className={cn("truncate font-mono text-[9px] uppercase leading-none text-muted-foreground", tileText)}>{label}</span>
        <strong className={cn("truncate text-sm font-semibold leading-tight sm:text-base", tileText)}>{value ?? "-"}</strong>
        {hint ? <span className={cn("truncate font-mono text-[8px] leading-none text-muted-foreground", tileText)}>{hint}</span> : null}
      </div>
    )
  }

  if (variant === "bordered") {
    return (
      <div className={cn("min-w-0 overflow-hidden rounded-md border p-2", className)} title={title}>
        <span className="flex min-w-0 w-full max-w-full flex-col items-center gap-1 overflow-hidden text-center">
          <strong className={cn("text-lg font-semibold", tileText)}>{value ?? "-"}</strong>
          <span className={cn("text-xs text-muted-foreground", tileText)}>{label}</span>
          {hint ? <span className={cn("font-mono text-xs text-muted-foreground", tileText)}>{hint}</span> : null}
        </span>
      </div>
    )
  }

  const content = (
    <span className="flex min-w-0 w-full max-w-full flex-col items-start gap-1 overflow-hidden text-left">
      <span className={cn("font-mono text-xs uppercase text-muted-foreground", tileText)}>{label}</span>
      <strong className={cn("font-semibold", tileText, variant === "compact" ? "text-base" : "text-sm")}>{value ?? "-"}</strong>
      {hint ? <span className={cn("font-mono text-xs text-muted-foreground", tileText)}>{hint}</span> : null}
    </span>
  )

  if (onClick) {
    return (
      <Button
        type="button"
        variant={active ? "secondary" : "outline"}
        className={cn("h-auto min-w-0 max-w-full justify-start overflow-hidden px-3 py-2", className)}
        title={title}
        onClick={onClick}
      >
        {content}
      </Button>
    )
  }

  return (
    <div
      className={cn(
        variant === "compact" && "min-w-0 overflow-hidden rounded-md border px-3 py-2",
        variant === "plain" && "min-w-0 overflow-hidden border-b pb-3",
        className,
      )}
      title={title}
    >
      {content}
    </div>
  )
}
