import { useState } from "react"
import { ChevronDownIcon } from "lucide-react"
import { McpstoreAsciiBanner } from "@/components/shared/mcpstore-ascii-banner"
import { MarkIcon } from "@/components/shared/mark-icon"
import { PathText } from "@/components/shared/path-text"
import { useI18n } from "@/lib/i18n-context"
import { absoluteApiBase, getApiBase } from "@/lib/api/backend"

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

function formatHeroStats(statItems: ReturnType<typeof useStatItems>, stats: HomeHeroStats) {
  return statItems
    .map(({ key, label }) => `${label}=${stats.loading ? "-" : stats[key]}`)
    .join(" · ")
}

export function HomeHero({ stats }: { stats: HomeHeroStats }) {
  const { t } = useI18n()
  const statItems = useStatItems()
  const statsLine = formatHeroStats(statItems, stats)
  const [collapsed, setCollapsed] = useState(false)
  const subtitle = absoluteApiBase(getApiBase())

  if (collapsed) {
    return (
      <section className="py-2">
        <button
          type="button"
          className="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-md px-2 py-1 text-left hover:bg-muted"
          onClick={() => setCollapsed(false)}
          aria-label={t("expandBanner")}
        >
          <span className="flex min-w-0 items-baseline gap-2">
            <span className="shrink-0 font-mono text-xs uppercase text-muted-foreground">
              {t("store")}
            </span>
            <span className="flex shrink-0 items-center gap-1.5">
              <MarkIcon />
              <strong className="truncate font-mono">mcpstore</strong>
            </span>
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
    <section className="overflow-hidden py-3">
      <button
        type="button"
        className="block w-full rounded-md text-left hover:bg-muted/30"
        onClick={() => setCollapsed(true)}
        title={t("collapseBanner")}
      >
        <McpstoreAsciiBanner />
      </button>
    </section>
  )
}
