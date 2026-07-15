import type { HTMLAttributes } from "react"

import { cn } from "@/lib/utils"

type TwoPanePageProps = HTMLAttributes<HTMLDivElement> & {
  variant?: "full" | "page"
}

export function TwoPanePage({ variant = "full", className, ...props }: TwoPanePageProps) {
  return (
    <div
      className={cn(
        variant === "full" &&
          "grid h-full min-h-0 min-w-0 grid-cols-1 grid-rows-[minmax(0,1fr)_minmax(0,1fr)] gap-[18px] md:grid-cols-[minmax(280px,0.44fr)_minmax(0,1fr)] md:grid-rows-1",
        variant === "page" && "grid min-h-[calc(100vh-124px)] grid-cols-1 gap-4 lg:grid-cols-[minmax(280px,0.44fr)_minmax(0,1fr)]",
        className,
      )}
      {...props}
    />
  )
}
