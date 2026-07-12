import type { ComponentProps, ReactNode } from "react"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

export function CatalogTabsList({ className, ...props }: ComponentProps<typeof TabsList>) {
  return <TabsList className={cn("w-full", className)} {...props} />
}

export function CatalogTabTrigger({
  className,
  label,
  children,
  ...props
}: ComponentProps<typeof TabsTrigger> & {
  label: string
  children: ReactNode
}) {
  return (
    <TabsTrigger
      title={label}
      aria-label={label}
      className={cn("min-w-0 flex-1 px-1 @min-[360px]:gap-1.5 @min-[360px]:px-2", className)}
      {...props}
    >
      {children}
      <span className="hidden truncate @min-[360px]:inline">{label}</span>
    </TabsTrigger>
  )
}
