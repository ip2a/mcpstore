import type { ComponentProps, ReactNode } from "react"

import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

type ScrollPaneProps = ComponentProps<typeof ScrollArea> & {
  innerClassName?: string
  children: ReactNode
}

export function ScrollPane({ className, innerClassName, children, ...props }: ScrollPaneProps) {
  return (
    <ScrollArea className={cn("min-h-0", className)} {...props}>
      <div className={cn("block w-full min-w-0 pe-1", innerClassName)}>{children}</div>
    </ScrollArea>
  )
}
