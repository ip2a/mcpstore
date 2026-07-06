import { useState } from "react"
import { ChevronDownIcon } from "lucide-react"
import { MCPSTORE_ASCII, randomAsciiBannerColor } from "@/lib/ascii-banner"
import type { CacheBackend } from "@/lib/api"

const API_BASE = import.meta.env.VITE_MCPSTORE_API_BASE || "/api"

export type HomeHeroStats = {
  loading: boolean
  services: number
  connected: number
  disconnected: number
  connecting: number
  error: number
  tools: number
  agents: number
}

const STAT_ITEMS: Array<{ key: keyof Omit<HomeHeroStats, "loading">; label: string }> = [
  { key: "services", label: "Services" },
  { key: "connected", label: "Connected" },
  { key: "disconnected", label: "Disconnected" },
  { key: "connecting", label: "Connecting" },
  { key: "error", label: "Error" },
  { key: "tools", label: "Tools" },
  { key: "agents", label: "Agents" },
]

function PathText({ value, className }: { value: string; className?: string }) {
  return (
    <span className={`font-mono text-xs text-muted-foreground break-all ${className || ""}`} title={value}>
      {value}
    </span>
  )
}

function StatValue({ loading, value }: { loading: boolean; value: number }) {
  return <span className="font-mono text-sm font-medium tabular-nums">{loading ? "-" : value}</span>
}

function HeroStats({ stats }: { stats: HomeHeroStats }) {
  return (
    <dl className="m-0 grid w-full grid-cols-2 gap-x-4 gap-y-1.5 sm:grid-cols-3">
      {STAT_ITEMS.map(({ key, label }) => (
        <div key={key} className="flex min-w-0 items-baseline justify-between gap-2">
          <dt className="truncate text-xs text-muted-foreground">{label}</dt>
          <dd className="m-0">
            <StatValue loading={stats.loading} value={stats[key]} />
          </dd>
        </div>
      ))}
    </dl>
  )
}

export function HomeHero({ backend, stats }: { backend?: CacheBackend; stats: HomeHeroStats }) {
  const [collapsed, setCollapsed] = useState(false)
  const [bannerColor, setBannerColor] = useState(randomAsciiBannerColor)
  const subtitle = backend ? `cache: ${backend} · api: ${API_BASE}` : `api: ${API_BASE}`

  function setHeroCollapsed(next: boolean) {
    setBannerColor(randomAsciiBannerColor())
    setCollapsed(next)
  }

  if (collapsed) {
    return (
      <section className="border-y py-2">
        <button
          type="button"
          className="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-md px-2 py-1 text-left hover:bg-muted"
          onClick={() => setHeroCollapsed(false)}
          aria-label="Expand banner"
        >
          <span className="grid min-w-0 grid-cols-[auto_minmax(80px,auto)_minmax(120px,1fr)] items-baseline gap-2">
            <span className="font-mono text-xs uppercase text-muted-foreground">Store</span>
            <strong className="truncate">mcpstore</strong>
            <PathText
              value={`${subtitle} · services ${stats.loading ? "-" : stats.services}`}
              className="min-w-0 truncate"
            />
          </span>
          <ChevronDownIcon aria-hidden="true" className="shrink-0" />
        </button>
      </section>
    )
  }

  return (
    <section className="grid grid-cols-1 items-start gap-3 overflow-hidden border-y py-3 md:grid-cols-[minmax(0,7fr)_minmax(280px,3fr)]">
      <button
        type="button"
        className="group grid min-w-0 place-items-center overflow-hidden rounded-md text-left hover:bg-muted/30"
        onClick={() => setHeroCollapsed(true)}
        title="Collapse"
        style={{ ["--ascii-banner-color" as string]: bannerColor }}
      >
        <pre
          className="m-0 overflow-hidden whitespace-pre font-mono text-[clamp(9px,1.05vw,14px)] font-black leading-none text-[var(--ascii-banner-color)] transition-opacity duration-150 group-hover:opacity-90"
          style={{ textShadow: "0 0 14px color-mix(in srgb, var(--ascii-banner-color) 28%, transparent)" }}
        >
          {MCPSTORE_ASCII}
        </pre>
      </button>
      <div className="flex min-w-0 flex-col items-start justify-start gap-3 self-stretch border-l pl-4">
        <div className="flex w-full min-w-0 flex-col gap-2">
          <h1 className="m-0 w-full truncate text-left text-2xl font-semibold leading-none">mcpstore</h1>
          <PathText value={subtitle} className="block w-full min-w-0 text-left" />
        </div>
        <HeroStats stats={stats} />
      </div>
    </section>
  )
}
