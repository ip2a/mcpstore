import { useMemo, useRef } from "react"
import { useQuery } from "@tanstack/react-query"

import { Card, CardContent } from "@/components/ui/card"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useI18n } from "@/lib/i18n-context"
import { getRecentFrames, type FrameRecord } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const COLS = 53
const ROWS = 7
/** One cell = one second; left-top oldest → right-bottom newest. */
const TOTAL_CELLS = COLS * ROWS
const FETCH_LIMIT = 2000
const REFRESH_MS = 60_000
const NO_RESPONSE_KEY = "no-response"
const NO_RESPONSE_COLOR = "#dc2626"

/** Known MCP methods → distinct hex; unknown methods hash into FALLBACK_PALETTE. */
const METHOD_COLOR: Record<string, string> = {
  initialize: "#6366f1",
  ping: "#94a3b8",
  "tools/list": "#5b5bd5",
  "tools/call": "#818cf8",
  "notifications/tools/list_changed": "#a5b4fc",
  "resources/list": "#0ea5e9",
  "resources/read": "#38bdf8",
  "resources/templates/list": "#7dd3fc",
  "resources/subscribe": "#0284c7",
  "resources/unsubscribe": "#0369a1",
  "notifications/resources/list_changed": "#bae6fd",
  "notifications/resources/updated": "#e0f2fe",
  "prompts/list": "#f59e0b",
  "prompts/get": "#d97706",
  "notifications/prompts/list_changed": "#fcd34d",
  "logging/setLevel": "#a78bfa",
  "completion/complete": "#c084fc",
  "roots/list": "#14b8a6",
  "notifications/roots/list_changed": "#5eead4",
}

const FALLBACK_PALETTE = [
  "#e879f9",
  "#f472b6",
  "#fb7185",
  "#fb923c",
  "#a3e635",
  "#34d399",
  "#2dd4bf",
  "#22d3ee",
  "#60a5fa",
  "#c4b5fd",
]

type CellEvent = {
  method: string
  when: string
  face: string
  direction: string
  noResponse: boolean
  /** Color bucket key: method name, or no-response when unanswered. */
  colorKey: string
}

type GridCell = {
  sec: number
  color: string | null
  /** Legend / single-tone label; mixed keys joined with " + ". */
  label: string | null
  events: CellEvent[]
}

type LegendItem = {
  color: string
  label: string
}

function hashMethod(method: string): number {
  let h = 0
  for (let i = 0; i < method.length; i++) h = (h * 31 + method.charCodeAt(i)) >>> 0
  return h
}

function colorForKey(key: string): string {
  if (key === NO_RESPONSE_KEY) return NO_RESPONSE_COLOR
  const known = METHOD_COLOR[key]
  if (known) return known
  return FALLBACK_PALETTE[hashMethod(key) % FALLBACK_PALETTE.length]
}

function parseHex(hex: string): [number, number, number] {
  const h = hex.replace("#", "")
  return [
    Number.parseInt(h.slice(0, 2), 16),
    Number.parseInt(h.slice(2, 4), 16),
    Number.parseInt(h.slice(4, 6), 16),
  ]
}

function mixColors(keys: string[]): string {
  if (keys.length === 1) return colorForKey(keys[0])
  let r = 0
  let g = 0
  let b = 0
  for (const key of keys) {
    const [cr, cg, cb] = parseHex(colorForKey(key))
    r += cr
    g += cg
    b += cb
  }
  const n = keys.length
  const toHex = (v: number) => Math.round(v / n).toString(16).padStart(2, "0")
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`
}

function methodLabel(frame: FrameRecord): string {
  return frame.method ?? frame.kind
}

function buildAnsweredIds(frames: FrameRecord[]): Set<string> {
  const answered = new Set<string>()
  for (const frame of frames) {
    if ((frame.kind === "response" || frame.kind === "error") && frame.jsonrpc_id) {
      answered.add(frame.jsonrpc_id)
    }
  }
  return answered
}

function isNoResponse(frame: FrameRecord, answered: Set<string>): boolean {
  if (frame.kind === "error") return true
  if (frame.kind === "notification") return false
  if (frame.kind === "request") {
    return frame.jsonrpc_id == null || !answered.has(frame.jsonrpc_id)
  }
  return true
}

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

function formatSecLabel(sec: number): string {
  return new Date(sec * 1000).toLocaleTimeString()
}

const EMPTY_GRID = {
  cells: [] as GridCell[],
  legend: [] as LegendItem[],
  requestCount: 0,
  noResponseCount: 0,
  frameCount: 0,
}

/** Row-major second grid anchored to the latest request second (not wall clock). */
function buildGrid(frames: FrameRecord[]): {
  cells: GridCell[]
  legend: LegendItem[]
  requestCount: number
  noResponseCount: number
  frameCount: number
} {
  const answered = buildAnsweredIds(frames)
  const visible = frames.filter((frame) => frame.kind !== "response")
  if (visible.length === 0) return EMPTY_GRID

  let endSec = Number.NEGATIVE_INFINITY
  for (const frame of visible) {
    const sec = Math.floor(frame.ts_unix_ms / 1000)
    if (sec > endSec) endSec = sec
  }
  const startSec = endSec - (TOTAL_CELLS - 1)

  const bySec = new Map<number, CellEvent[]>()
  let requestCount = 0
  let noResponseCount = 0
  let frameCount = 0

  for (const frame of visible) {
    const sec = Math.floor(frame.ts_unix_ms / 1000)
    if (sec < startSec || sec > endSec) continue
    frameCount++

    const method = methodLabel(frame)
    const unanswered = isNoResponse(frame, answered)
    if (frame.kind === "request" || frame.kind === "notification") requestCount++
    if (unanswered) noResponseCount++

    const event: CellEvent = {
      method,
      when: new Date(frame.ts_unix_ms).toLocaleTimeString(),
      face: frame.face,
      direction: frame.direction,
      noResponse: unanswered,
      colorKey: unanswered ? NO_RESPONSE_KEY : method,
    }
    const bucket = bySec.get(sec)
    if (bucket) bucket.push(event)
    else bySec.set(sec, [event])
  }

  const legendMap = new Map<string, string>()
  const cells: GridCell[] = Array.from({ length: TOTAL_CELLS }, (_, index) => {
    const sec = startSec + index
    const events = bySec.get(sec) ?? []
    if (events.length === 0) {
      return { sec, color: null, label: null, events: [] }
    }

    const keys = [...new Set(events.map((event) => event.colorKey))].sort((a, b) =>
      a.localeCompare(b),
    )
    const color = mixColors(keys)
    const label = keys.join(" + ")
    legendMap.set(label, color)
    return { sec, color, label, events }
  })

  const legend: LegendItem[] = [...legendMap.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([label, color]) => ({ label, color }))

  return { cells, legend, requestCount, noResponseCount, frameCount }
}

function legendDisplayLabel(label: string, noResponseLabel: string): string {
  return label
    .split(" + ")
    .map((part) => (part === NO_RESPONSE_KEY ? noResponseLabel : part))
    .join(" + ")
}

function HeatmapCell({
  cell,
  noResponseLabel,
}: {
  cell: GridCell
  noResponseLabel: string
}) {
  if (cell.color == null || cell.events.length === 0) {
    return <div className="min-h-0 min-w-0 rounded-[4px] bg-muted" />
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className="min-h-0 min-w-0 rounded-[4px]"
          style={{ backgroundColor: cell.color }}
        />
      </TooltipTrigger>
      <TooltipContent side="top" sideOffset={6} className="max-w-xs">
        <p className="font-medium">{formatSecLabel(cell.sec)}</p>
        <ul className="mt-1 flex flex-col gap-1">
          {cell.events.map((event, index) => (
            <li key={`${event.when}-${event.method}-${index}`} className="text-background/75">
              <span className="font-medium text-background">{event.method}</span>
              {" · "}
              {event.face} · {event.direction}
              {event.noResponse ? ` · ${noResponseLabel}` : ""}
            </li>
          ))}
        </ul>
      </TooltipContent>
    </Tooltip>
  )
}

export function OverviewUsageHeatmapCard({ className }: { className?: string }) {
  const { t } = useI18n()
  const pinnedFrames = useRef<FrameRecord[]>([])
  const { data: frames } = useQuery({
    queryKey: queryKeys.gatewayFramesRecent("all", FETCH_LIMIT),
    queryFn: () => getRecentFrames({ limit: FETCH_LIMIT }),
    refetchInterval: REFRESH_MS,
  })

  // Anchor to last non-empty traffic; skip updates while the stream stays empty.
  const sourceFrames = useMemo(() => {
    const next = frames ?? []
    if (next.some((frame) => frame.kind !== "response")) {
      pinnedFrames.current = next
    }
    return pinnedFrames.current
  }, [frames])

  const grid = useMemo(() => buildGrid(sourceFrames), [sourceFrames])
  const noResponseLabel = t("homeFrameLegNoResponse")

  return (
    <Card className={cn("gap-0", className)}>
      <CardContent className="flex min-h-0 flex-1 flex-col px-4 pb-4 pt-4">
        <div className="grid shrink-0 grid-cols-3 gap-6 pb-4">
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">{t("homeFrameStatTotal")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5">
              <span className="text-3xl font-semibold tabular-nums tracking-tight">
                {formatCompact(grid.frameCount)}
              </span>
              <span className="text-sm text-muted-foreground">{t("homeFrameStatFrames")}</span>
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {t("homeFrameStatWindow", { count: TOTAL_CELLS })}
            </p>
          </div>
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">{t("homeFrameStatRequests")}</p>
            <p className="mt-1 text-3xl font-semibold tabular-nums tracking-tight">
              {formatCompact(grid.requestCount)}
            </p>
          </div>
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">{t("homeFrameStatNoResponse")}</p>
            <p className="mt-1 flex flex-wrap items-baseline gap-x-1.5">
              <span
                className={cn(
                  "text-3xl font-semibold tabular-nums tracking-tight",
                  grid.noResponseCount > 0 && "text-[#dc2626]",
                )}
              >
                {formatCompact(grid.noResponseCount)}
              </span>
            </p>
          </div>
        </div>

        {grid.frameCount === 0 ? (
          <p className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
            {t("homeFrameEmpty")}
          </p>
        ) : (
          <div
            className="grid min-h-0 w-full min-w-0 flex-1 gap-[3px]"
            style={{
              gridTemplateColumns: `repeat(${COLS}, minmax(0, 1fr))`,
              gridTemplateRows: `repeat(${ROWS}, minmax(0, 1fr))`,
            }}
          >
            {grid.cells.map((cell) => (
              <HeatmapCell
                key={cell.sec}
                cell={cell}
                noResponseLabel={noResponseLabel}
              />
            ))}
          </div>
        )}

        {grid.legend.length > 0 ? (
          <div className="mt-3 flex shrink-0 flex-wrap items-center justify-start gap-x-4 gap-y-2">
            {grid.legend.map((item) => (
              <div key={item.label} className="flex items-center gap-1.5">
                <div
                  className="size-3 shrink-0 rounded-[4px]"
                  style={{ backgroundColor: item.color }}
                />
                <span className="text-xs text-muted-foreground">
                  {legendDisplayLabel(item.label, noResponseLabel)}
                </span>
              </div>
            ))}
          </div>
        ) : null}
      </CardContent>
    </Card>
  )
}
