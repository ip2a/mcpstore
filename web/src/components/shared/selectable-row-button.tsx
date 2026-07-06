import type { ButtonHTMLAttributes, ReactNode } from "react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

type SelectableRowButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "title"> & {
  leading?: ReactNode
  meta?: ReactNode
  selected?: boolean
  title: ReactNode
  trailing?: ReactNode
}

export function SelectableRowButton({ className, leading, meta, selected = false, title, trailing, ...props }: SelectableRowButtonProps) {
  return (
    <Button
      type="button"
      variant={selected ? "secondary" : "outline"}
      className={cn("h-auto min-h-11 justify-start px-3 py-2 text-left", className)}
      {...props}
    >
      <span className={cn("grid w-full min-w-0 items-center gap-3", leading ? "grid-cols-[auto_minmax(0,1fr)_auto]" : "grid-cols-[minmax(0,1fr)_auto]")}>
        {leading}
        {meta ? (
          <span className="flex min-w-0 flex-col gap-1">
            <strong className="truncate text-sm font-medium">{title}</strong>
            <span className="truncate font-mono text-xs text-muted-foreground">{meta}</span>
          </span>
        ) : (
          <strong className="truncate text-sm font-medium">{title}</strong>
        )}
        {trailing}
      </span>
    </Button>
  )
}
