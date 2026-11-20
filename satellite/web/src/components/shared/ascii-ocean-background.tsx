import { useEffect, useRef } from "react"
import { useTheme } from "next-themes"

import { cn } from "@/lib/utils"

type AsciiOceanController = {
  update: (opts: Record<string, unknown>) => void
  destroy: () => void
}

/**
 * Decorative ASCII shark ocean (MIT: stackedlol/ascii-sharks).
 * Non-interactive; sits behind settings tab content.
 */
export function AsciiOceanBackground({ className }: { className?: string }) {
  const hostRef = useRef<HTMLDivElement>(null)
  const ctrlRef = useRef<AsciiOceanController | null>(null)
  const { resolvedTheme } = useTheme()
  const theme = resolvedTheme === "dark" ? "dark" : "light"

  useEffect(() => {
    const host = hostRef.current
    if (!host) return

    let cancelled = false
    let resizeObserver: ResizeObserver | undefined

    void import("@/vendor/ascii-ocean.js").then(({ mountAsciiOcean }) => {
      if (cancelled || !hostRef.current) return
      const el = hostRef.current
      const ctrl = mountAsciiOcean(el, {
        showComposer: false,
        theme,
        fishCount: 120,
        height: Math.max(1, el.clientHeight),
      }) as AsciiOceanController
      ctrlRef.current = ctrl

      resizeObserver = new ResizeObserver(() => {
        ctrl.update({ height: Math.max(1, el.clientHeight) })
      })
      resizeObserver.observe(el)
    })

    return () => {
      cancelled = true
      resizeObserver?.disconnect()
      ctrlRef.current?.destroy()
      ctrlRef.current = null
    }
    // Mount once; theme updates via the effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional mount-once
  }, [])

  useEffect(() => {
    ctrlRef.current?.update({ theme })
  }, [theme])

  return (
    <div
      ref={hostRef}
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 z-0 overflow-hidden opacity-[0.34]",
        className,
      )}
    />
  )
}
