import { useEffect, useState } from "react"
import { DatabaseIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { DetailHeader } from "@/components/shared/detail-header"
import { JsonBlock } from "@/components/shared/json-block"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { cacheHealth, cacheInspect, type CacheBackend } from "@/lib/api"

export function CacheView(props: { backend?: CacheBackend; revision: number; onRefreshDashboard: () => Promise<void>; onSwitch: () => void }) {
  const [healthReport, setHealthReport] = useState<unknown>(null)
  const [inspectReport, setInspectReport] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function loadCache() {
    setLoading(true)
    try {
      setError(null)
      const [health, inspect] = await Promise.all([cacheHealth(), cacheInspect()])
      setHealthReport(health)
      setInspectReport(inspect)
      await props.onRefreshDashboard()
    } catch (err) {
      const message = err instanceof Error ? err.message : "缓存加载失败"
      setError(message)
      toast.error(message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadCache()
  }, [props.revision])

  return (
    <>
      <DetailHeader
        eyebrow="缓存管理"
        title="Cache Storage"
        actions={
          <>
            <Button variant="outline" onClick={loadCache} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            <Button onClick={props.onSwitch}>
              <DatabaseIcon data-icon="inline-start" />
              切换
            </Button>
          </>
        }
      />

      <MetricGrid columns="three">
        <MetricTile variant="compact" label="Current backend" value={props.backend || "unknown"} />
        <MetricTile variant="compact" label="Health keys" value={countKeys(healthReport)} />
        <MetricTile variant="compact" label="Inspect keys" value={countKeys(inspectReport)} />
      </MetricGrid>

      <section className="grid gap-4 lg:grid-cols-2">
        <PanelCard>
          <SectionHeading title="Health" titleAs="h2" description="/cache/health" className="border-b-0 pb-0" />
          {error ? (
            <PageError title="Cache health failed to load" message={error} onRefresh={loadCache} />
          ) : loading && !healthReport ? (
            <PageSkeleton />
          ) : (
            <JsonBlock value={healthReport || {}} />
          )}
        </PanelCard>
        <PanelCard>
          <SectionHeading title="Inspect" titleAs="h2" description="/cache/inspect" className="border-b-0 pb-0" />
          {error ? (
            <PageError title="Cache inspect failed to load" message={error} onRefresh={loadCache} />
          ) : loading && !inspectReport ? (
            <PageSkeleton />
          ) : (
            <JsonBlock value={inspectReport || {}} />
          )}
        </PanelCard>
      </section>
    </>
  )
}

function countKeys(value: unknown) {
  return value && typeof value === "object" ? Object.keys(value).length : 0
}
