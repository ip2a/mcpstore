import type { ComponentProps, ReactNode } from "react"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

type CatalogTabsListProps = ComponentProps<typeof TabsList> & {
  /** Equal thirds with Tools centered; outer tabs extend slightly toward the edges. */
  layout?: "flex" | "catalog"
}

export function CatalogTabsList({
  className,
  layout = "flex",
  ...props
}: CatalogTabsListProps) {
  return (
    <TabsList
      className={cn(
        "@container w-full min-w-0",
        layout === "flex" && "justify-start",
        layout === "catalog" &&
          "grid grid-cols-[minmax(0,1.12fr)_minmax(0,1fr)_minmax(0,1.12fr)]",
        className,
      )}
      {...props}
    />
  )
}

export function CatalogTabTrigger({
  className,
  label,
  children,
  variant = "icon",
  ...props
}: ComponentProps<typeof TabsTrigger> & {
  label: string
  children?: ReactNode
  variant?: "icon" | "text"
}) {
  return (
    <TabsTrigger
      title={label}
      aria-label={label}
      className={cn(
        variant === "text"
          ? "min-w-0 w-full flex-1 basis-0 px-2"
          : "min-w-0 flex-1 px-1 @min-[360px]:gap-1.5 @min-[360px]:px-2",
        className,
      )}
      {...props}
    >
      {variant === "icon" ? children : null}
      <span className={cn(variant === "text" ? null : "hidden truncate @min-[360px]:inline")}>
        {label}
      </span>
    </TabsTrigger>
  )
}
