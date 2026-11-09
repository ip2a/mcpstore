import { useEffect, useRef, useState, type ComponentProps } from "react"
import { RefreshCwIcon } from "lucide-react"

import { cn } from "@/lib/utils"

/** Tailwind `animate-spin` is 1s per turn — hold at least one full turn. */
const MIN_SPIN_MS = 1000

/** Keep spinning until `active` ends and at least one full rotation has played. */
export function useHeldSpin(active: boolean, minMs = MIN_SPIN_MS) {
  const [spinning, setSpinning] = useState(false)
  const startedAtRef = useRef(0)

  useEffect(() => {
    if (active) {
      startedAtRef.current = Date.now()
      setSpinning(true)
      return
    }

    let cancelled = false
    const remaining = Math.max(0, minMs - (Date.now() - startedAtRef.current))
    const timer = window.setTimeout(() => {
      if (!cancelled) setSpinning(false)
    }, remaining)

    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [active, minMs])

  return spinning
}

export function RefreshCwSpinIcon({
  spinning,
  className,
  ...props
}: ComponentProps<typeof RefreshCwIcon> & { spinning: boolean }) {
  const held = useHeldSpin(spinning)
  return <RefreshCwIcon className={cn(held && "animate-spin", className)} {...props} />
}
