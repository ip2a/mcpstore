import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { ArrowLeftIcon, LinkIcon, RefreshCwIcon, Trash2Icon, UnlinkIcon } from "lucide-react"
import { toast } from "sonner"

import { DetailHeader } from "@/components/shared/detail-header"
import { JsonBlock } from "@/components/shared/json-block"
import { MetaLine } from "@/components/shared/meta-line"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { ToolCard } from "@/components/shared/tool-card"
import { Button } from "@/components/ui/button"
import { formatDateTime } from "@/lib/format"
import { queryKeys } from "@/lib/query-keys"
import { toolKey } from "@/lib/tool-info"
import { serviceInfo, serviceStatus, type ServiceEntry, type ToolInfo } from "@/lib/api"

export function ServiceDetailView(props: {
  service: ServiceEntry
  busy: string | null
  refreshToken?: number
  onBack: () => void
  onRunTool: (tool: ToolInfo) => void
  onToolDetail: (tool: ToolInfo) => void
  onConnect: () => void
  onDisconnect: () => void
  onRestart: () => void
  onDelete: () => void
}) {
  const [detailError, setDetailError] = useState<string | null>(null)
  const detailQuery = useQuery({ enabled: false, queryKey: queryKeys.service(props.service.name), queryFn: () => serviceInfo(props.service.name) })
  const statusQuery = useQuery({
    enabled: false,
    queryKey: queryKeys.serviceStatus(props.service.name),
    queryFn: () => serviceStatus(props.service.name).catch(() => null),
  })
  const detail = detailQuery.data
  const statusReport = statusQuery.data
  const error = detailQuery.error || detailError
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : "服务详情加载失败"
  const loading = detailQuery.isFetching || statusQuery.isFetching
  const service = detail || props.service
  const endpoint = service.url || service.command || "-"
  const tools = service.tools || []

  async function loadDetail() {
    try {
      setDetailError(null)
      const [nextDetail] = await Promise.all([detailQuery.refetch(), statusQuery.refetch()])
      if (nextDetail.error) throw nextDetail.error
    } catch (err) {
      const message = err instanceof Error ? err.message : "服务详情加载失败"
      if (!detailQuery.error) setDetailError(message)
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadDetail()
  }, [props.refreshToken, props.service.name])

  return (
    <>
      <DetailHeader
        badges={<ServiceStatusBadge status={service.status} />}
        eyebrow="服务详情"
        meta={
          <div className="flex flex-wrap gap-2 text-sm text-muted-foreground">
            <span>tools · {tools.length}</span>
            <span>added · {formatDateTime(service.added_time)}</span>
          </div>
        }
        title={service.name}
        actions={
          <>
            <Button variant="outline" onClick={props.onBack}>
              <ArrowLeftIcon data-icon="inline-start" />
              Back
            </Button>
            <Button variant="outline" onClick={loadDetail} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              刷新
            </Button>
            {service.status === "Connected" ? (
              <Button variant="outline" onClick={props.onDisconnect} disabled={Boolean(props.busy)}>
                <UnlinkIcon data-icon="inline-start" />
                Disconnect
              </Button>
            ) : (
              <Button onClick={props.onConnect} disabled={Boolean(props.busy)}>
                <LinkIcon data-icon="inline-start" />
                Connect
              </Button>
            )}
            <Button variant="outline" onClick={props.onRestart} disabled={Boolean(props.busy)}>
              <RefreshCwIcon data-icon="inline-start" />
              Restart
            </Button>
            <Button variant="destructive" onClick={props.onDelete} disabled={Boolean(props.busy)}>
              <Trash2Icon data-icon="inline-start" />
              Delete
            </Button>
          </>
        }
      />

      <MetricGrid columns="four">
        <MetricTile variant="compact" label="Name" value={service.name} title={service.name} />
        <MetricTile variant="compact" label="Endpoint" value={String(endpoint)} title={String(endpoint)} />
        <MetricTile variant="compact" label="Agent" value={String(service.agent_id || "store")} />
        <MetricTile variant="compact" label="Tools" value={String(tools.length)} />
      </MetricGrid>

      <PanelCard>
        <SectionHeading title="Service Info" titleAs="h2" className="border-b-0 pb-0" actions={<ServiceStatusBadge status={service.status} />} />
        <div className="grid gap-4 text-sm md:grid-cols-2">
          <MetaLine label="Transport" value={String(service.transport || "unknown")} valueClassName="font-mono" />
          <MetaLine label="Original" value={String(service.original_name || service.name)} valueClassName="font-mono" />
          <MetaLine label="Added" value={formatDateTime(service.added_time)} valueClassName="font-mono" />
          <MetaLine label="Command" value={String(service.command || "-")} valueClassName="font-mono" />
          <MetaLine label="URL" value={String(service.url || "-")} valueClassName="font-mono" />
        </div>
      </PanelCard>

      <section className="flex flex-col gap-3">
        <SectionHeading title="Tool List" titleAs="h2" description={`${tools.length} items`} className="border-b-0 pb-0" />
        {tools.length ? (
          <div className="grid gap-4 lg:grid-cols-2">
            {tools.map((tool) => (
              <ToolCard
                key={toolKey(tool)}
                tool={tool}
                sourceLabel={service.name}
                onRun={() => props.onRunTool(tool)}
                onDetail={() => props.onToolDetail(tool)}
              />
            ))}
          </div>
        ) : (
          <PageEmpty title="No tools found" description="Tool definitions will appear after the service is connected." onRefresh={loadDetail} />
        )}
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <PanelCard>
          <SectionHeading title="Status" titleAs="h2" className="border-b-0 pb-0" />
          {error ? <PageError title="Service status failed to load" message={errorMessage} onRefresh={loadDetail} /> : <JsonBlock value={statusReport || { status: service.status || "Unknown" }} />}
        </PanelCard>
        <PanelCard>
          <SectionHeading title="Raw Detail" titleAs="h2" className="border-b-0 pb-0" />
          <JsonBlock value={service} />
        </PanelCard>
      </section>
    </>
  )
}
