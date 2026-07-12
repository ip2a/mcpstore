import { useState } from "react"
import { ChevronDownIcon } from "lucide-react"
import { PathText } from "@/components/shared/path-text"
import { MCPSTORE_ASCII, randomAsciiBannerColor } from "@/lib/ascii-banner"
import { useI18n } from "@/lib/i18n-context"
import type { CacheBackend } from "@/lib/api"

const API_BASE = import.meta.env.VITE_MCPSTORE_API_BASE || "/api"

export type HomeHeroStats = {
  loading: boolean
  services: number
  connecting: number
  agents: number
}

function useStatItems() {
  const { t } = useI18n()
  return [
    { key: "services" as const, label: t("services") },
    { key: "connecting" as const, label: t("connecting") },
    { key: "agents" as const, label: t("agents") },
  ]
}

function formatHeroStats(
  statItems: ReturnType<typeof useStatItems>,
  stats: HomeHeroStats,
) {
  return statItems.map(({ key, label }) => `${label}=${stats.loading ? "-" : stats[key]}`).join(" · ")
}

function HeroStats({ stats }: { stats: HomeHeroStats }) {
  const statItems = useStatItems()
  return <PathText value={formatHeroStats(statItems, stats)} className="block w-full min-w-0 text-left" wrap="all" />
}

export function HomeHero({ backend, stats }: { backend?: CacheBackend; stats: HomeHeroStats }) {
  const { t } = useI18n()
  const statItems = useStatItems()
  const statsLine = formatHeroStats(statItems, stats)
  const [collapsed, setCollapsed] = useState(false)
  const [bannerColor, setBannerColor] = useState(randomAsciiBannerColor)
  const subtitle = t("homeHeroSubtitle", { backend: backend || "", api: API_BASE })

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
          aria-label={t("expandBanner")}
        >
          <span className="flex min-w-0 items-baseline gap-2">
            <span className="shrink-0 font-mono text-xs uppercase text-muted-foreground">{t("store")}</span>
            <strong className="shrink-0 truncate">mcpstore</strong>
            <PathText
              value={`${subtitle} · ${statsLine}`}
              className="min-w-0 truncate"
              wrap="truncate"
            />
          </span>
          <ChevronDownIcon aria-hidden="true" className="shrink-0" />
        </button>
      </section>
    )
  }

  return (
    <section className="grid grid-cols-1 gap-3 overflow-hidden border-y py-3 md:grid-cols-[minmax(0,7fr)_minmax(280px,3fr)] md:items-stretch">
      <button
        type="button"
        className="group grid min-h-0 min-w-0 place-items-center self-stretch overflow-hidden rounded-md text-left hover:bg-muted/30"
        onClick={() => setHeroCollapsed(true)}
        title={t("collapseBanner")}
        style={{ ["--ascii-banner-color" as string]: bannerColor }}
      >
        <pre
          className="m-0 overflow-hidden whitespace-pre font-mono text-[clamp(10px,1.28vw,17px)] font-black leading-none text-[var(--ascii-banner-color)] transition-opacity duration-150 group-hover:opacity-90"
          style={{ textShadow: "0 0 14px color-mix(in srgb, var(--ascii-banner-color) 28%, transparent)" }}
        >
          {MCPSTORE_ASCII}
        </pre>
      </button>
      <div className="flex min-w-0 flex-col items-start justify-start gap-3 self-stretch border-l pl-4">
        <div className="flex w-full min-w-0 flex-col gap-2">
          <h1 className="m-0 w-full truncate text-left text-2xl font-semibold leading-none">mcpstore</h1>
          <PathText value={subtitle} className="block w-full min-w-0 text-left" wrap="all" />
        </div>
        <HeroStats stats={stats} />
      </div>
    </section>
  )
}
