import { forwardRef, type ComponentProps, type ReactNode } from "react"

import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

type ScrollPaneProps = ComponentProps<typeof ScrollArea> & {
  innerClassName?: string
  children: ReactNode
}

export const ScrollPane = forwardRef<React.ComponentRef<typeof ScrollArea>, ScrollPaneProps>(
  function ScrollPane({ className, innerClassName, children, ...props }, ref) {
    return (
      <ScrollArea ref={ref} className={cn("min-h-0 overflow-hidden", className)} {...props}>
        <div className={cn("block w-full min-w-0 pe-1", innerClassName)}>{children}</div>
      </ScrollArea>
    )
  },
)
