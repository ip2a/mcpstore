import { useState } from "react"

import { MCPSTORE_ASCII, randomAsciiBannerColor } from "@/lib/ascii-banner"
import { cn } from "@/lib/utils"

export function McpstoreAsciiBanner({ className }: { className?: string }) {
  const [bannerColor] = useState(randomAsciiBannerColor)

  return (
    <div
      className={cn(
        "grid min-h-0 min-w-0 place-items-center overflow-hidden rounded-md bg-muted/25 px-2 py-4",
        className,
      )}
      style={{ ["--ascii-banner-color" as string]: bannerColor }}
    >
      <pre
        className="m-0 max-w-full overflow-hidden whitespace-pre font-mono text-[clamp(9px,1.1vw,15px)] font-black leading-none text-[var(--ascii-banner-color)]"
        style={{ textShadow: "0 0 14px color-mix(in srgb, var(--ascii-banner-color) 28%, transparent)" }}
      >
        {MCPSTORE_ASCII}
      </pre>
    </div>
  )
}
