import { useState, type ReactNode } from "react"
import { ChevronRightIcon } from "lucide-react"

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { cn } from "@/lib/utils"

export function FormSectionCollapsible({
  children,
  defaultOpen = false,
  icon,
  title,
}: {
  children: ReactNode
  defaultOpen?: boolean
  icon?: ReactNode
  title: string
}) {
  const [open, setOpen] = useState(defaultOpen)

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-md border">
      <CollapsibleTrigger className="flex w-full items-center gap-2 px-4 py-3 text-left text-sm font-medium transition-colors hover:bg-muted/50">
        <ChevronRightIcon
          className={cn("size-4 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
        />
        {icon}
        <span>{title}</span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="flex flex-col gap-4 border-t px-4 py-4">{children}</div>
      </CollapsibleContent>
    </Collapsible>
  )
}
