import { useEffect, useLayoutEffect, useRef, type RefObject } from "react"

import type { ServiceInstance } from "@/lib/api"

function shouldPreserveScrollForBusy(busy: string | null) {
  return (
    busy?.startsWith("connect:") ||
    busy?.startsWith("disconnect:") ||
    busy?.startsWith("restart:") ||
    busy === "check:instances"
  )
}

function getScrollViewport(root: HTMLElement | null) {
  const viewport = root?.querySelector('[data-slot="scroll-area-viewport"]')
  return viewport instanceof HTMLElement ? viewport : null
}

export function usePreserveServiceListScroll({
  busy,
  listRootRef,
  services,
}: {
  busy: string | null
  listRootRef: RefObject<HTMLElement | null>
  services: ServiceInstance[]
}) {
  const servicesRevision = useRef(0)
  const pendingRestore = useRef<{ scrollTop: number; baselineRevision: number } | null>(null)

  useEffect(() => {
    servicesRevision.current += 1
  }, [services])

  useEffect(() => {
    if (!shouldPreserveScrollForBusy(busy)) return
    const viewport = getScrollViewport(listRootRef.current)
    if (!viewport) return
    pendingRestore.current = {
      scrollTop: viewport.scrollTop,
      baselineRevision: servicesRevision.current,
    }
  }, [busy, listRootRef])

  useLayoutEffect(() => {
    const pending = pendingRestore.current
    if (!pending || busy !== null) return
    if (servicesRevision.current === pending.baselineRevision) return

    const viewport = getScrollViewport(listRootRef.current)
    if (!viewport) return

    viewport.scrollTop = pending.scrollTop
    pendingRestore.current = null
  }, [busy, listRootRef, services])
}
