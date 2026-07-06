import type { HTMLAttributes, ReactNode } from "react"

import { cn } from "@/lib/utils"

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined
}

type EntityRowProps = HTMLAttributes<HTMLElement> & {
  actions?: ReactNode
  actionsProps?: HTMLAttributes<HTMLDivElement> & DataAttributes
  asChild?: boolean
  children: ReactNode
  selected?: boolean
  variant?: "article" | "inline"
}

export function EntityRow({ actions, actionsProps, asChild = false, children, className, selected = false, variant = "article", ...props }: EntityRowProps) {
  if (asChild) {
    return (
      <article className={cn("rounded-md border px-3 py-3", selected && "bg-muted", className)} data-selected={selected ? "true" : "false"} {...props}>
        {children}
      </article>
    )
  }

  const content = (
    <>
      <div className="min-w-0">{children}</div>
      {actions ? (
        <div {...actionsProps} className={cn("flex flex-wrap justify-start gap-2 md:justify-end", actionsProps?.className)}>
          {actions}
        </div>
      ) : null}
    </>
  )

  if (variant === "inline") {
    return (
      <div className={cn("grid gap-3 border-b py-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center", className)} {...props}>
        {content}
      </div>
    )
  }

  return (
    <article className={cn("rounded-md border px-3 py-3", selected && "bg-muted", className)} data-selected={selected ? "true" : "false"} {...props}>
      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-start">{content}</div>
    </article>
  )
}
