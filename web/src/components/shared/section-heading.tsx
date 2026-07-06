import type { HTMLAttributes, ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined
}

type SectionHeadingProps = Omit<HTMLAttributes<HTMLDivElement>, "title"> & {
  actions?: ReactNode
  actionsProps?: HTMLAttributes<HTMLDivElement> & DataAttributes
  badge?: ReactNode
  description?: ReactNode
  eyebrow?: ReactNode
  title: ReactNode
  titleAs?: "h1" | "h2" | "strong"
  variant?: "compact" | "page"
}

export function SectionHeading({
  actions,
  actionsProps,
  badge,
  className,
  description,
  eyebrow,
  title,
  titleAs = "strong",
  variant = "compact",
  ...props
}: SectionHeadingProps) {
  const Title = titleAs

  return (
    <div
      className={cn(
        "grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-start",
        variant === "compact" && "border-b pb-2",
        className,
      )}
      {...props}
    >
      <div className="flex min-w-0 flex-col gap-1">
        {eyebrow ? <Badge variant="secondary" className="w-fit">{eyebrow}</Badge> : null}
        <Title className={cn(variant === "page" ? "text-2xl font-semibold" : "text-sm font-medium")}>{title}</Title>
        {description ? <span className="text-sm text-muted-foreground">{description}</span> : null}
      </div>
      {actions || badge !== undefined ? (
        <div {...actionsProps} className={cn("flex flex-wrap justify-start gap-2 md:justify-end", actionsProps?.className)}>
          {actions}
          {badge !== undefined ? <Badge variant="outline">{badge}</Badge> : null}
        </div>
      ) : null}
    </div>
  )
}
