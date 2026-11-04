import { useState } from "react"

import { ASCII_LETTERS, randomAsciiBannerColor } from "@/lib/ascii-banner"
import { cn } from "@/lib/utils"

export function McpstoreAsciiBanner({ className }: { className?: string }) {
  const [bannerColor] = useState(randomAsciiBannerColor)
  const letters = ['M', 'C', 'P', 'S', 'T', 'O', 'R', 'E']

  return (
    <div
      className={cn(
        "grid min-h-0 min-w-0 place-items-center overflow-hidden rounded-md bg-muted/25 px-2 py-4",
        className,
      )}
      style={{ ["--ascii-banner-color" as string]: bannerColor }}
    >
      <div className="flex items-start gap-[0.5em] font-mono text-[clamp(9px,1.1vw,15px)] font-black leading-[1.1]">
        {letters.map((letter) => (
          <pre
            key={letter}
            className="m-0 whitespace-pre text-[var(--ascii-banner-color)]"
            style={{ textShadow: "0 0 14px color-mix(in srgb, var(--ascii-banner-color) 28%, transparent)" }}
          >
            {ASCII_LETTERS[letter]}
          </pre>
        ))}
      </div>
    </div>
  )
}
