import { useState, type ReactNode } from "react"

import { cn } from "@/lib/utils"

type McpIconEntry = { src?: unknown }

export function firstIconSrc(icons: unknown): string | null {
  if (!Array.isArray(icons)) return null
  for (const entry of icons) {
    const src = (entry as McpIconEntry)?.src
    if (typeof src === "string" && src) return src
  }
  return null
}

/** Renders the first MCP icon if present, otherwise the given fallback. Falls back again if the image fails to load. */
export function McpIcon({
  icons,
  fallback,
  className,
}: {
  icons: unknown
  fallback: ReactNode
  className?: string
}) {
  const src = firstIconSrc(icons)
  const [broken, setBroken] = useState(false)
  if (!src || broken) return <>{fallback}</>
  return (
    <img
      key={src}
      src={src}
      alt=""
      aria-hidden
      className={cn("shrink-0 object-contain", className)}
      onError={() => setBroken(true)}
    />
  )
}
