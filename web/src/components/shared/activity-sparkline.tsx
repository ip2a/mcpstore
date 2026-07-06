import { useId, useMemo } from "react"

import { cn } from "@/lib/utils"

type ActivitySparklineProps = {
  className?: string
  height?: number
  isLoading?: boolean
  title?: string
  values: number[]
}

const CHART_WIDTH = 320
const CHART_INSET = 2

type SparkPoint = { x: number; y: number }

function buildSparklinePoints(values: number[], height: number): SparkPoint[] {
  const max = Math.max(...values, 1)
  const top = 3
  const bottom = height - 3
  const innerHeight = bottom - top
  const count = values.length

  return values.map((value, index) => {
    const x = count <= 1 ? CHART_WIDTH / 2 : CHART_INSET + (index / Math.max(count - 1, 1)) * (CHART_WIDTH - CHART_INSET * 2)
    const y = top + innerHeight * (1 - value / max)
    return { x, y }
  })
}

function buildFlatLine(y: number, inset = CHART_INSET) {
  return `M ${inset} ${y} L ${CHART_WIDTH - inset} ${y}`
}

function buildSmoothLine(points: SparkPoint[]) {
  if (points.length === 0) return ""
  if (points.length === 1) return buildFlatLine(points[0].y)
  if (points.length === 2) {
    return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`
  }

  let path = `M ${points[0].x} ${points[0].y}`
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[Math.max(index - 1, 0)]
    const current = points[index]
    const next = points[index + 1]
    const after = points[Math.min(index + 2, points.length - 1)]
    const control1x = current.x + (next.x - previous.x) / 6
    const control1y = current.y + (next.y - previous.y) / 6
    const control2x = next.x - (after.x - current.x) / 6
    const control2y = next.y - (after.y - current.y) / 6
    path += ` C ${control1x} ${control1y}, ${control2x} ${control2y}, ${next.x} ${next.y}`
  }
  return path
}

function buildSparklinePath(values: number[], height: number) {
  if (values.length === 0) return { area: "", line: "" }

  const bottom = height - 3
  const points = buildSparklinePoints(values, height)
  const line = buildSmoothLine(points)

  if (points.length === 1) {
    return {
      line,
      area: `${line} L ${CHART_WIDTH - CHART_INSET} ${bottom} L ${CHART_INSET} ${bottom} Z`,
    }
  }

  return {
    line,
    area: `${line} L ${points[points.length - 1].x} ${bottom} L ${points[0].x} ${bottom} Z`,
  }
}

export function ActivitySparkline({ className, height = 36, isLoading, title, values }: ActivitySparklineProps) {
  const gradientId = useId().replace(/:/g, "")
  const paths = useMemo(() => buildSparklinePath(values, height), [height, values])

  if (isLoading) {
    return <div className={cn("animate-pulse rounded-full bg-muted/50", className)} style={{ height }} />
  }

  if (values.length === 0) {
    return <div className={cn("rounded-full bg-muted/15", className)} style={{ height }} title={title} />
  }

  return (
    <svg
      viewBox={`0 0 ${CHART_WIDTH} ${height}`}
      preserveAspectRatio="none"
      className={cn("block w-full overflow-visible text-primary", className)}
      style={{ height }}
      role="img"
      aria-label={title ?? "Activity sparkline"}
    >
      {title ? <title>{title}</title> : null}
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.34" />
          <stop offset="55%" stopColor="currentColor" stopOpacity="0.12" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.01" />
        </linearGradient>
      </defs>
      <path d={paths.area} className={`fill-[url(#${gradientId})]`} vectorEffect="non-scaling-stroke" />
      <path
        d={paths.line}
        className="fill-none stroke-current"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
        style={{ filter: "drop-shadow(0 0 0.35px currentColor)" }}
      />
    </svg>
  )
}
