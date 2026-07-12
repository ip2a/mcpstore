import { ClipboardIcon, EyeIcon, RefreshCwIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import {
  ToolAnnotationsSection,
  ToolInputSchemaSection,
  ToolMetaSection,
  ToolOutputSchemaSection,
} from "@/components/shared/tool-capability-sections"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import {
  toolDetailSectionAside,
  toolDetailSectionGrid,
  toolDetailSectionLabel,
} from "@/components/shared/tool-detail-section-layout"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useToolsRegistry } from "@/features/tools/use-tools-registry"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem, type ServiceEntry, type ToolInfo } from "@/lib/api"
import { getToolOutputSchema, getToolSchema, getToolServiceName, toolKey } from "@/lib/tool-info"
import type { ToolDetailState, ToolDialogState } from "@/features/tools/tool-dialogs"
import { cn } from "@/lib/utils"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  onRunTool: (state: ToolDialogState) => void
  onToolDetail: (state: ToolDetailState) => void
}) {
  const { t } = useI18n()
  const {
    agentId,
    agentIds,
    error,
    errorMessage,
    loadTools,
    loading,
    makeRunner,
    query,
    scope,
    selectedTool,
    selectedToolKey,
    serviceName,
    setAgentId,
    setQuery,
    setScope,
    setSelectedToolKey,
    setServiceName,
    visibleTools,
  } = useToolsRegistry({ agents: props.agents })

  const sourceLabel = selectedTool
    ? makeRunner(selectedTool).sourceLabel
    : scope === "agent"
      ? t("agentScopeLabel", { agentId: agentId || "-" })
      : t("store")
  const schema = selectedTool
    ? (getToolSchema(selectedTool) as { properties?: Record<string, unknown>; required?: string[] })
    : null
  const outputSchema = selectedTool
    ? (getToolOutputSchema(selectedTool) as { properties?: Record<string, unknown> })
    : null
  const paramCount = Object.keys(schema?.properties || {}).length
  const outputCount = Object.keys(outputSchema?.properties || {}).length

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("tool")}</p>
            <h2 className="mt-1 truncate text-lg font-semibold">{t("toolRegistryTitle")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("toolRegistryDescription", {
                count: visibleTools.length,
                scope: scope === "agent" ? t("agentScopeLabel", { agentId: agentId || "-" }) : t("storeScope"),
              })}
            </p>
          </div>
        </section>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3">
          <SectionHeading
            title={t("toolList")}
            titleAs="h2"
            description={t("items", { count: visibleTools.length })}
            descriptionPlacement="inline"
            className="border-b-0 pb-0"
          />
          {error ? (
            <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
          ) : loading && !visibleTools.length ? (
            <PageSkeleton />
          ) : visibleTools.length ? (
            <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
              {visibleTools.map((tool) => {
                const key = toolKey(tool)
                const itemSchema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
                const itemParamCount = Object.keys(itemSchema.properties || {}).length
                return (
                  <SelectableRowButton
                    key={key}
                    meta={`${getToolServiceName(tool) || t("store")} · ${t("paramCount", { count: itemParamCount })}`}
                    onClick={() => setSelectedToolKey(key)}
                    selected={key === selectedToolKey}
                    title={tool.name}
                    trailing={itemSchema.required?.length ? <Badge variant="outline">{itemSchema.required.length}</Badge> : null}
                  />
                )
              })}
            </ScrollPane>
          ) : (
            <PageEmpty title={t("noTools")} description={t("noToolsScopeDescription")} onRefresh={loadTools} />
          )}
        </div>

        <section className="mt-3 shrink-0 border-t pt-4">
          <SectionHeading
            title={t("filters")}
            titleAs="h2"
            description={t("matched", { count: visibleTools.length })}
            className="border-b-0 pb-3"
          />
          <FieldGroup>
            <Field>
              <FieldLabel>{t("search")}</FieldLabel>
              <SearchBox placeholder={t("searchTools")} value={query} onChange={setQuery} />
            </Field>
            <Field>
              <FieldLabel>{t("scope")}</FieldLabel>
              <Select value={scope} onValueChange={setScope}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="store">{t("store")}</SelectItem>
                    <SelectItem value="agent">{t("agent")}</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <FieldLabel>{t("agent")}</FieldLabel>
              <Select
                value={agentId || "none"}
                onValueChange={(value) => setAgentId(value === "none" ? "" : value)}
                disabled={scope !== "agent"}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="none">{t("noAgent")}</SelectItem>
                    {agentIds.map((id) => (
                      <SelectItem key={id} value={id}>
                        {id}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <FieldLabel>{t("service")}</FieldLabel>
              <Select value={serviceName} onValueChange={setServiceName}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="all">{t("allServices")}</SelectItem>
                    {props.services.map((service) => (
                      <SelectItem key={service.name} value={service.name}>
                        {service.name}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
        </section>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        <ToolPreviewHeader
          loading={loading}
          selectedTool={selectedTool}
          onCopy={selectedTool ? () => copyTool(selectedTool) : undefined}
          onDetail={selectedTool ? () => props.onToolDetail(makeRunner(selectedTool)) : undefined}
          onRefresh={loadTools}
          onRun={selectedTool ? () => props.onRunTool(makeRunner(selectedTool)) : undefined}
        />

        {selectedTool ? <ToolSummarySection tool={selectedTool} sourceLabel={sourceLabel} /> : null}

        <MetricGrid columns="four">
          <MetricTile variant="compact" label={t("params")} value={String(paramCount)} hint={t("inputFields")} />
          <MetricTile
            variant="compact"
            label={t("required")}
            value={String(schema?.required?.length || 0)}
            hint={t("mandatory")}
          />
          <MetricTile variant="compact" label={t("output")} value={String(outputCount)} hint={t("outputFields")} />
          <MetricTile variant="compact" label={t("source")} value={sourceLabel} title={sourceLabel} hint={t("callScope")} />
        </MetricGrid>

        <ScrollPane className="flex-1">
          {error && !selectedTool ? (
            <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
          ) : loading && !selectedTool ? (
            <PageSkeleton />
          ) : selectedTool ? (
            <ToolDetailPane tool={selectedTool} />
          ) : (
            <PageEmpty
              title={t("noToolSelected")}
              description={t("noToolSelectedDescription")}
              onRefresh={loadTools}
            />
          )}
        </ScrollPane>
      </PanelCard>
    </TwoPanePage>
  )
}

function ToolPreviewHeader({
  loading,
  onCopy,
  onDetail,
  onRefresh,
  onRun,
  selectedTool,
}: {
  loading: boolean
  onCopy?: () => void
  onDetail?: () => void
  onRefresh: () => void
  onRun?: () => void
  selectedTool: ToolInfo | null | undefined
}) {
  const { t } = useI18n()
  const title = selectedTool?.name || t("noToolSelected")
  const hideTitle = Boolean(selectedTool)

  return (
    <div className={cn("flex flex-wrap items-center gap-3 border-b pb-2", hideTitle ? "justify-end" : "justify-between")}>
      {!hideTitle ? (
        <div className="flex min-w-0 flex-col gap-1">
          <strong className="truncate font-mono text-sm font-medium" title={title}>
            {title}
          </strong>
        </div>
      ) : null}
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {onRun ? (
          <Button size="sm" onClick={onRun}>
            <WrenchIcon data-icon="inline-start" />
            {t("run")}
          </Button>
        ) : null}
        {onDetail ? (
          <Button size="sm" variant="outline" onClick={onDetail}>
            <EyeIcon data-icon="inline-start" />
            {t("details")}
          </Button>
        ) : null}
        {onCopy ? (
          <Button size="sm" variant="outline" onClick={onCopy}>
            <ClipboardIcon data-icon="inline-start" />
            {t("copy")}
          </Button>
        ) : null}
        <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
          <RefreshCwIcon data-icon="inline-start" />
          {t("refresh")}
        </Button>
      </div>
    </div>
  )
}

function ToolSummarySection({ sourceLabel, tool }: { sourceLabel: string; tool: ToolInfo }) {
  return (
    <section className="border-b pb-4">
      <div className={toolDetailSectionGrid}>
        <div className={toolDetailSectionAside}>
          <h2 className={cn(toolDetailSectionLabel, "font-mono")} title={tool.name}>
            {tool.name}
          </h2>
        </div>
        <div className="flex flex-col items-end gap-2 text-right">
          <Badge variant="secondary">{sourceLabel || getToolServiceName(tool) || "store"}</Badge>
          <ToolDescriptionBlock description={tool.description} showLabel={false} />
        </div>
      </div>
    </section>
  )
}

function ToolDetailPane({ tool }: { tool: ToolInfo }) {
  return (
    <div className="flex min-w-0 flex-col">
      <ToolInputSchemaSection tool={tool} />
      <ToolOutputSchemaSection tool={tool} />
      <ToolAnnotationsSection tool={tool} />
      <ToolMetaSection tool={tool} />
    </div>
  )
}

async function copyTool(tool: ToolInfo) {
  await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
  toast.success("Tool copied")
}
