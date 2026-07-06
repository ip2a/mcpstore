import type { HTMLAttributes, ReactNode } from "react"

import { cn } from "@/lib/utils"

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined
}

type DetailHeaderProps = Omit<HTMLAttributes<HTMLElement>, "title"> & {
  actions?: ReactNode
  actionsPlacement?: "below" | "inline"
  actionsProps?: HTMLAttributes<HTMLDivElement> & DataAttributes
  badges?: ReactNode
  description?: ReactNode
  eyebrow?: ReactNode
  meta?: ReactNode
  separated?: boolean
  title: ReactNode
  titleSize?: "default" | "large"
}

export function DetailHeader({
  actions,
  actionsPlacement = "inline",
  actionsProps,
  badges,
  className,
  description,
  eyebrow,
  meta,
  separated = false,
  title,
  titleSize = "default",
  ...props
}: DetailHeaderProps) {
  const titleText = typeof title === "string" ? title : undefined
  const actionsNode = actions ? (
    <div
      {...actionsProps}
      className={cn(
        "flex flex-wrap gap-2",
        actionsPlacement === "inline" ? "justify-start md:justify-end" : "justify-start",
        actionsProps?.className,
      )}
    >
      {actions}
    </div>
  ) : null

  return (
    <section className={cn("flex flex-col gap-3", separated && "border-b pb-4", className)} {...props}>
      <div className={cn(actionsPlacement === "inline" && actions ? "grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-start" : "flex flex-col gap-2")}>
        <div className="flex min-w-0 flex-col gap-2">
          {badges ? <div className="flex flex-wrap gap-2">{badges}</div> : null}
          {eyebrow ? <p className="font-mono text-xs uppercase text-muted-foreground">{eyebrow}</p> : null}
          <h1 className={cn("line-clamp-2 font-semibold leading-snug [overflow-wrap:anywhere]", titleSize === "large" ? "text-3xl" : "text-2xl")} title={titleText}>
            {title}
          </h1>
          {description ? <div className="text-muted-foreground [overflow-wrap:anywhere]">{description}</div> : null}
          {meta ? <div className="w-full">{meta}</div> : null}
        </div>
        {actionsPlacement === "inline" ? actionsNode : null}
      </div>
      {actionsPlacement === "below" ? actionsNode : null}
    </section>
  )
}
