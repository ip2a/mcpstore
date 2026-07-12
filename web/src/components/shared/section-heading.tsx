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
  descriptionPlacement?: "below" | "inline"
  eyebrow?: ReactNode
  title: ReactNode
  titleAddon?: ReactNode
  titleAs?: "h1" | "h2" | "strong"
  variant?: "compact" | "page"
}

export function SectionHeading({
  actions,
  actionsProps,
  badge,
  className,
  description,
  descriptionPlacement = "below",
  eyebrow,
  title,
  titleAddon,
  titleAs = "strong",
  variant = "compact",
  ...props
}: SectionHeadingProps) {
  const Title = titleAs
  const titleClassName = cn(variant === "page" ? "text-2xl font-semibold" : "text-sm font-medium")

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
        {description && descriptionPlacement === "inline" ? (
          <div className="flex min-w-0 items-center justify-between gap-2">
            <Title className={titleClassName}>{title}</Title>
            <span className="shrink-0 text-sm text-muted-foreground">{description}</span>
          </div>
        ) : (
          <>
            <div className={cn(titleAddon && "flex min-w-0 items-center gap-2")}>
              <Title className={cn(titleClassName, titleAddon && "shrink-0")}>{title}</Title>
              {titleAddon}
            </div>
            {description ? <span className="text-sm text-muted-foreground">{description}</span> : null}
          </>
        )}
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
