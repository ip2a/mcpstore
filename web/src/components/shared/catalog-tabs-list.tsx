import type { ComponentProps, ReactNode } from "react"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

export function CatalogTabsList({ className, ...props }: ComponentProps<typeof TabsList>) {
  return <TabsList className={cn("@container w-fit max-w-full", className)} {...props} />
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
        "flex-none shrink-0",
        variant === "text" ? "px-3" : "px-1 @min-[360px]:gap-1.5 @min-[360px]:px-2",
        className,
      )}
      {...props}
    >
      {variant === "icon" ? children : null}
      <span className={cn(variant === "text" ? "inline" : "hidden truncate @min-[360px]:inline")}>{label}</span>
    </TabsTrigger>
  )
}
