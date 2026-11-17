import { useEffect, useState } from "react"

import { getGatewayRequests, type GatewayRequestRecord } from "@/lib/api"

/**
 * Long-polling loop: waits for new records or the 25s timeout, then continues;
 * resets and re-fetches on backend restart (seq goes backwards).
 */
export function useGatewayRequestFlow(): GatewayRequestRecord[] {
  const [records, setRecords] = useState<GatewayRequestRecord[]>([])
  useEffect(() => {
    let cancelled = false
    let after = 0
    void (async () => {
      while (!cancelled) {
        try {
          const page = await getGatewayRequests(after, 25_000)
          if (cancelled) return
          if (page.latest_seq < after) {
            after = 0
            setRecords([])
          }
          if (page.requests.length > 0) {
            after = page.latest_seq
            setRecords((prev) => {
              const byId = new Map(prev.map((record) => [record.call_id, record]))
              for (const record of page.requests) byId.set(record.call_id, record)
              return [...byId.values()].sort((a, b) => a.seq - b.seq).slice(-200)
            })
          } else if (!page.running) {
            // When the gateway is not running the backend returns immediately
            // (no 25s long wait); throttle here or it spins at full speed.
            await new Promise((resolve) => setTimeout(resolve, 3_000))
          }
        } catch {
          await new Promise((resolve) => setTimeout(resolve, 3_000))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])
  return records
}
