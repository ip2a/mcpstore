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
        <div {...actionsProps} className={cn("flex flex-wrap justify-start gap-2 @min-[36rem]:justify-end", actionsProps?.className)}>
          {actions}
        </div>
      ) : null}
    </>
  )

  if (variant === "inline") {
    return (
      <div className={cn("@container border-b py-3", className)} {...props}>
        <div className="grid gap-3 @min-[36rem]:grid-cols-[minmax(0,1fr)_auto] @min-[36rem]:items-center">
          {content}
        </div>
      </div>
    )
  }

  return (
    <article className={cn("@container rounded-md border px-3 py-3", selected && "bg-muted", className)} data-selected={selected ? "true" : "false"} {...props}>
      <div className="grid gap-3 @min-[36rem]:grid-cols-[minmax(0,1fr)_auto] @min-[36rem]:items-start">{content}</div>
    </article>
  )
}
