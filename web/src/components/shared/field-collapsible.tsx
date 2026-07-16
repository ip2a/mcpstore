import { useState, type ReactNode } from "react"
import { ChevronRightIcon } from "lucide-react"

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Field } from "@/components/ui/field"
import { cn } from "@/lib/utils"

export function FieldCollapsible({
  children,
  defaultOpen = false,
  title,
}: {
  children: ReactNode
  defaultOpen?: boolean
  title: string
}) {
  const [open, setOpen] = useState(defaultOpen)

  return (
    <Field>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          className={cn(
            "flex w-fit cursor-pointer items-center gap-1.5 text-sm font-medium leading-snug select-none",
            "transition-colors hover:text-foreground/80",
          )}
        >
          <ChevronRightIcon
            className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
          />
          <span>{title}</span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="pt-2">{children}</div>
        </CollapsibleContent>
      </Collapsible>
    </Field>
  )
}
