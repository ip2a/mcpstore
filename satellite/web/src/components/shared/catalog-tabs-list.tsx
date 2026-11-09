import type { ComponentProps } from "react"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

type CatalogTabsListProps = ComponentProps<typeof TabsList> & {
  /** Equal thirds with Tools centered; outer tabs extend slightly toward the edges. */
  layout?: "flex" | "catalog"
}

export function CatalogTabsList({ className, layout = "flex", ...props }: CatalogTabsListProps) {
  return (
    <TabsList
      className={cn(
        "@container w-full min-w-0",
        layout === "flex" && "justify-start",
        layout === "catalog" && "grid grid-cols-[minmax(0,1.12fr)_minmax(0,1fr)_minmax(0,1.12fr)]",
        className,
      )}
      {...props}
    />
  )
}

export function CatalogTabTrigger({
  className,
  label,
  ...props
}: ComponentProps<typeof TabsTrigger> & {
  label: string
}) {
  return (
    <TabsTrigger
      title={label}
      aria-label={label}
      className={cn("min-w-0 w-full flex-1 basis-0 px-2", className)}
      {...props}
    >
      <span className="truncate">{label}</span>
    </TabsTrigger>
  )
}
