import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"

import {
  getFrameHistory,
  getFrameSessions,
  getRecentFrames,
  type FrameRecord,
  type FrameSessionSummary,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

function pickLatestForInstance(
  sessions: FrameSessionSummary[],
  instanceId: string,
  notBeforeMs = 0,
): FrameSessionSummary | null {
  let best: FrameSessionSummary | null = null
  for (const session of sessions) {
    if (session.instance_id !== instanceId) continue
    // Prefer a session that started (or last spoke) after the connect watermark.
    if (notBeforeMs > 0) {
      if (session.first_ts_unix_ms < notBeforeMs && session.last_ts_unix_ms < notBeforeMs) {
        continue
      }
    }
    if (!best || session.last_ts_unix_ms > best.last_ts_unix_ms) best = session
  }
  return best
}

function framesForInstanceSince(
  frames: FrameRecord[],
  instanceId: string,
  notBeforeMs: number,
): FrameRecord[] {
  const matched = frames.filter(
    (frame) =>
      frame.instance_id === instanceId &&
      (notBeforeMs <= 0 || frame.ts_unix_ms >= notBeforeMs),
  )
  if (matched.length === 0) return []
  // Stick to the newest session among matches so a reconnect doesn't mix timelines.
  const newestSessionId = matched[matched.length - 1]!.session_id
  return matched.filter((frame) => frame.session_id === newestSessionId)
}

/**
 * Service-card session frames.
 * - Idle open: one-shot latest session + history.
 * - Connecting: poll recent upstream frames and stream them as they land (backend
 *   already writes live; no extra daemon stream endpoint needed).
 */
function summaryFromFrames(
  frames: FrameRecord[],
  instanceId: string,
): FrameSessionSummary | null {
  if (frames.length === 0) return null
  const first = frames[0]!
  const last = frames[frames.length - 1]!
  let requestBytes = 0
  let responseBytes = 0
  for (const frame of frames) {
    if (frame.direction === "up") requestBytes += frame.size_bytes
    else responseBytes += frame.size_bytes
  }
  return {
    session_id: last.session_id,
    face: "upstream",
    caller: last.caller,
    peer: last.peer,
    instance_id: instanceId,
    first_ts_unix_ms: first.ts_unix_ms,
    last_ts_unix_ms: last.ts_unix_ms,
    frame_count: frames.length,
    request_bytes: requestBytes,
    response_bytes: responseBytes,
  }
}

/**
 * Service-card session frames.
 * - Idle open: one-shot latest session + history.
 * - Connecting: poll recent upstream frames and stream them as they land (backend
 *   already writes live; no extra daemon stream endpoint needed).
 * - Live → idle: seed idle caches from the last live window so the rail does not
 *   flash empty / skeleton, then soft-refetch history for the shared key.
 */
export function useServiceCardSessionFrames(
  instanceId: string,
  enabled: boolean,
  options?: {
    /** While connecting, poll and keep selecting the newest frame. */
    followLatest?: boolean
    /** Ignore frames/sessions before this (ms); set when Connect is clicked. */
    notBeforeMs?: number
  },
) {
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const followLatest = options?.followLatest ?? false
  const notBeforeMs = options?.notBeforeMs ?? 0
  const live = followLatest || notBeforeMs > 0
  const prevSessionId = useRef<string | null>(null)
  const pinnedSessionId = useRef<string | null>(null)
  /** Survives live query-key change when the connect watermark is cleared. */
  const heldLiveFrames = useRef<FrameRecord[]>([])
  const wasLive = useRef(false)

  // Live connect: pull recent upstream frames frequently and filter client-side.
  // Backend tees each JSON-RPC message into SQLite as it happens — this is the
  // streaming surface without a new SSE endpoint.
  const liveRecentQuery = useQuery({
    enabled: enabled && live,
    queryKey: [
      ...queryKeys.gatewayFramesRecent("upstream", 400),
      "card-live",
      instanceId,
      notBeforeMs,
    ],
    queryFn: async () => {
      const recent = await getRecentFrames({ face: "upstream", limit: 400 })
      return framesForInstanceSince(recent, instanceId, notBeforeMs)
    },
    refetchInterval: enabled && live ? 300 : false,
    staleTime: 0,
  })

  const liveFrames = liveRecentQuery.data ?? []
  const liveSessionId = liveFrames[liveFrames.length - 1]?.session_id ?? null

  useEffect(() => {
    if (live && liveFrames.length > 0) heldLiveFrames.current = liveFrames
    if (live && liveSessionId) pinnedSessionId.current = liveSessionId
    if (!enabled) {
      pinnedSessionId.current = null
      heldLiveFrames.current = []
    }
  }, [enabled, live, liveFrames, liveSessionId])

  // Before paint: hand live frames to idle caches so the first idle render is warm.
  useLayoutEffect(() => {
    if (live) {
      wasLive.current = true
      return
    }
    if (!wasLive.current) return
    wasLive.current = false
    const held = heldLiveFrames.current
    const sid = pinnedSessionId.current || held[held.length - 1]?.session_id || null
    if (!sid || held.length === 0) return
    const sessionsKey = [...queryKeys.gatewayFrameSessions, "by-instance", instanceId] as const
    const historyKey = queryKeys.gatewayFrameHistory(sid)
    const summary = summaryFromFrames(held, instanceId)
    if (summary) queryClient.setQueryData(sessionsKey, summary)
    queryClient.setQueryData(historyKey, held)
    // Soft refresh shared history (also used by Traces) without clearing the seed.
    void queryClient.invalidateQueries({ queryKey: historyKey })
  }, [instanceId, live, queryClient])

  // Idle: resolve the latest session once (or the session we just streamed).
  const sessionsQuery = useQuery({
    enabled: enabled && !live,
    queryKey: [...queryKeys.gatewayFrameSessions, "by-instance", instanceId],
    queryFn: async () => {
      const page = await getFrameSessions({
        face: "upstream",
        hours: 24,
        limit: 100,
        offset: 0,
      })
      return pickLatestForInstance(page.sessions, instanceId)
    },
    staleTime: 30_000,
  })

  const idleSessionId = pinnedSessionId.current || sessionsQuery.data?.session_id || null

  const idleHistoryQuery = useQuery({
    enabled: enabled && !live && Boolean(idleSessionId),
    queryKey: queryKeys.gatewayFrameHistory(idleSessionId || "pending"),
    queryFn: () => getFrameHistory(idleSessionId!),
    staleTime: 30_000,
  })

  const frames = live
    ? liveFrames
    : (idleHistoryQuery.data ?? heldLiveFrames.current)
  const sessionId = live ? liveSessionId : idleSessionId
  useEffect(() => {
    if (prevSessionId.current === sessionId) return
    prevSessionId.current = sessionId
    setSelectedId(null)
  }, [sessionId])

  useEffect(() => {
    if (!enabled) return
    if (frames.length === 0) {
      setSelectedId(null)
      return
    }
    if (followLatest) {
      setSelectedId(frames[frames.length - 1]!.id)
      return
    }
    if (selectedId != null && frames.some((frame) => frame.id === selectedId)) return
    setSelectedId(frames[0]!.id)
  }, [enabled, frames, followLatest, selectedId])

  const selected: FrameRecord | null = useMemo(
    () => frames.find((frame) => frame.id === selectedId) ?? null,
    [frames, selectedId],
  )

  // Never skeleton over frames we already have (live stream or seeded idle).
  const loading = frames.length === 0 && (live
    ? liveRecentQuery.isLoading
    : (!idleSessionId && sessionsQuery.isLoading) ||
      (Boolean(idleSessionId) && idleHistoryQuery.isLoading))

  return {
    frames,
    loading,
    fetching: live
      ? liveRecentQuery.isFetching
      : sessionsQuery.isFetching || idleHistoryQuery.isFetching,
    noSession: enabled && !loading && frames.length === 0 && !sessionId,
    selected,
    selectedId,
    setSelectedId,
    sessionId,
    refresh: async () => {
      if (live) {
        await liveRecentQuery.refetch()
        return
      }
      await sessionsQuery.refetch()
      if (idleSessionId) await idleHistoryQuery.refetch()
    },
  }
}
