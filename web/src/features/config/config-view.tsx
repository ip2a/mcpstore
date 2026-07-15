import { useEffect, useMemo, useState } from "react"
import { BotIcon, ClipboardIcon, RefreshCwIcon, SettingsIcon, StoreIcon } from "lucide-react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import {
  toolDetailSectionAside,
  toolDetailSectionGrid,
  toolDetailSectionLabel,
} from "@/components/shared/tool-detail-section-layout"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { getAgentId } from "@/features/agents/model"
import { type AgentItem } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { useAgentConfigQuery, useStoreConfigQuery } from "@/features/config/queries"
import { cn } from "@/lib/utils"

export type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }

type ConfigScopeId = "store" | `agent:${string}`

const storeScope = {
  id: "store" as const,
  titleKey: "storeConfig" as const,
  path: "/config",
  descriptionKey: "storeConfigDescription" as const,
}

function toAgentScopeId(agentId: string): ConfigScopeId {
  return `agent:${agentId}`
}

function parseAgentScopeId(scopeId: ConfigScopeId) {
  return scopeId.startsWith("agent:") ? scopeId.slice("agent:".length) : ""
}

export function ConfigView(props: { agents: AgentItem[]; resetTarget: ResetTarget | null; onResetTarget: (target: ResetTarget | null) => void }) {
  const { t } = useI18n()
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const [selectedScopeId, setSelectedScopeId] = useState<ConfigScopeId>("store")
  const selectedAgentId = selectedScopeId === "store" ? "" : parseAgentScopeId(selectedScopeId)
  const storeConfigQuery = useStoreConfigQuery()
  const agentConfigQuery = useAgentConfigQuery(selectedAgentId)
  const storeConfig = storeConfigQuery.data
  const agentConfig = selectedAgentId ? agentConfigQuery.data : null
  const selectedScope = useMemo(() => {
    if (selectedScopeId === "store") {
      return {
        id: selectedScopeId,
        title: t(storeScope.titleKey),
        path: storeScope.path,
        description: t(storeScope.descriptionKey),
      }
    }
    const agentId = parseAgentScopeId(selectedScopeId)
    return {
      id: selectedScopeId,
      title: t("agentConfig"),
      path: `/config/agents/${agentId}`,
      description: t("agentConfigDescription", { agentId }),
      agentId,
    }
  }, [selectedScopeId, t])
  const activeConfig = selectedScopeId === "store" ? storeConfig : agentConfig
  const activeLoading = selectedScopeId === "store" ? storeConfigQuery.isFetching : agentConfigQuery.isFetching
  const activeError = selectedScopeId === "store" ? storeConfigQuery.error : agentConfigQuery.error
  const error = storeConfigQuery.error || agentConfigQuery.error
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : t("configLoadFailed")
  const loading = storeConfigQuery.isFetching || agentConfigQuery.isFetching
  const storeKeyCount = countKeys(storeConfig)
  const agentKeyCount = countKeys(agentConfig)

  useEffect(() => {
    if (selectedScopeId !== "store" && !agentIds.includes(parseAgentScopeId(selectedScopeId)) && agentIds[0]) {
      setSelectedScopeId(toAgentScopeId(agentIds[0]))
    }
  }, [agentIds, selectedScopeId])

  async function loadConfig() {
    try {
      const store = await storeConfigQuery.refetch()
      if (store.error) throw store.error
      if (!selectedAgentId) return
      const agent = await agentConfigQuery.refetch()
      if (agent.error) throw agent.error
    } catch (err) {
      const message = err instanceof Error ? err.message : t("configLoadFailed")
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadConfig()
  }, [selectedAgentId, props.resetTarget])

  function onReset() {
    if (selectedScopeId === "store") {
      props.onResetTarget({ scope: "store" })
      return
    }
    const agentId = parseAgentScopeId(selectedScopeId)
    if (agentId) props.onResetTarget({ scope: "agent", agentId })
  }

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("configLabel")}</p>
            <h2 className="mt-1 truncate text-lg font-semibold">{t("configuration")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("configScopesSummary", { count: 1 + agentIds.length })}
            </p>
          </div>
        </section>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3">
          <SectionHeading
            title={t("configScopes")}
            titleAs="h2"
            description={t("itemsCount", { count: 1 + agentIds.length })}
            descriptionPlacement="inline"
            className="border-b-0 pb-0"
          />
          <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
            <SelectableRowButton
              meta={storeScope.path}
              onClick={() => setSelectedScopeId("store")}
              selected={selectedScopeId === "store"}
              title={t(storeScope.titleKey)}
              trailing={<Badge variant="outline">{t("keysSuffix", { count: storeKeyCount })}</Badge>}
            />
            {agentIds.length ? (
              agentIds.map((agentId) => (
                <SelectableRowButton
                  key={agentId}
                  meta={`/config/agents/${agentId}`}
                  onClick={() => setSelectedScopeId(toAgentScopeId(agentId))}
                  selected={selectedScopeId === toAgentScopeId(agentId)}
                  title={agentId}
                  trailing={
                    <Badge variant="outline">
                      {selectedScopeId === toAgentScopeId(agentId) ? t("keysSuffix", { count: agentKeyCount }) : "-"}
                    </Badge>
                  }
                />
              ))
            ) : (
              <PageEmpty title={t("noAgents")} description={t("noAgentsDescription")} />
            )}
          </ScrollPane>
        </div>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        <ConfigPreviewHeader
          loading={loading}
          selectedScope={selectedScope}
          onCopy={activeConfig ? () => copyConfig(activeConfig, t) : undefined}
          onRefresh={loadConfig}
          onReset={onReset}
          resetDisabled={selectedScopeId !== "store" && !selectedAgentId}
        />

        <ConfigSummarySection
          scope={selectedScope}
          keyCount={selectedScopeId === "store" ? storeKeyCount : agentKeyCount}
        />

        <MetricGrid columns="four">
          <MetricTile
            variant="compact"
            label={t("scope")}
            value={selectedScopeId === "store" ? t("store") : t("agent")}
            hint={selectedScope.path}
          />
          <MetricTile variant="compact" label={t("store")} value={String(storeKeyCount)} hint="/config" />
          <MetricTile variant="compact" label={t("agents")} value={String(agentIds.length)} hint={t("registeredAgentScopes")} />
          <MetricTile
            variant="compact"
            label={t("keys")}
            value={String(selectedScopeId === "store" ? storeKeyCount : agentKeyCount)}
            hint={activeLoading ? t("loadingHint") : t("currentScope")}
          />
        </MetricGrid>

        <ScrollPane className="flex-1">
          {error && !activeConfig ? (
            <PageError title={t("configLoadFailed")} message={errorMessage} onRefresh={loadConfig} />
          ) : activeError && !activeConfig ? (
            <PageError
              title={t("configLoadFailed")}
              message={activeError instanceof Error ? activeError.message : String(activeError)}
              onRefresh={loadConfig}
            />
          ) : activeLoading && !activeConfig ? (
            <PageSkeleton />
          ) : (
            <ConfigDetailPane scope={selectedScope} value={activeConfig || {}} loading={activeLoading && !activeConfig} />
          )}
        </ScrollPane>
      </PanelCard>
    </TwoPanePage>
  )
}

function ConfigPreviewHeader({
  loading,
  onCopy,
  onRefresh,
  onReset,
  resetDisabled,
  selectedScope,
}: {
  loading: boolean
  onCopy?: () => void
  onRefresh: () => void
  onReset: () => void
  resetDisabled?: boolean
  selectedScope: { title: string; path: string }
}) {
  const { t } = useI18n()

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b pb-2">
      <div className="flex min-w-0 flex-col gap-1">
        <strong className="truncate font-mono text-sm font-medium" title={selectedScope.path}>
          {selectedScope.path}
        </strong>
      </div>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        <Button size="sm" variant="outline" onClick={onReset} disabled={resetDisabled}>
          <RefreshCwIcon data-icon="inline-start" />
          {t("reset")}
        </Button>
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

function ConfigSummarySection({
  keyCount,
  scope,
}: {
  keyCount: number
  scope: { title: string; description: string; agentId?: string }
}) {
  const { t } = useI18n()
  const Icon = scope.agentId ? BotIcon : StoreIcon

  return (
    <section className="border-b pb-4">
      <div className={toolDetailSectionGrid}>
        <div className={toolDetailSectionAside}>
          <h2 className={cn(toolDetailSectionLabel, "inline-flex items-center gap-2 font-mono")} title={scope.title}>
            <Icon className="size-4 shrink-0" />
            {scope.title}
          </h2>
        </div>
        <p className="text-right text-sm text-muted-foreground">
          {scope.description} · {t("keysSuffix", { count: keyCount })}
        </p>
      </div>
    </section>
  )
}

function ConfigDetailPane({
  loading,
  scope,
  value,
}: {
  loading: boolean
  scope: { title: string; path: string; agentId?: string }
  value: unknown
}) {
  const { t } = useI18n()
  if (loading) return <PageSkeleton />

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={scope.title} titleAs="h2" description={scope.path} className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("scope")}</dt>
            <dd className="font-mono">{scope.agentId ? `agent:${scope.agentId}` : "store"}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("endpoint")}</dt>
            <dd className="break-all font-mono">{scope.path}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("keys")}</dt>
            <dd>{countKeys(value)}</dd>
          </div>
        </dl>
      </section>
      <section className="pb-2">
        <SectionHeading title={t("configuration")} titleAs="h2" actions={<SettingsIcon className="size-4 text-muted-foreground" />} className="border-b-0 pb-3" />
        <JsonBlock value={value} />
      </section>
    </div>
  )
}

async function copyConfig(value: unknown, t: (key: string) => string) {
  await navigator.clipboard.writeText(JSON.stringify(value, null, 2))
  toast.success(t("copied"))
}

function countKeys(value: unknown) {
  return value && typeof value === "object" ? Object.keys(value).length : 0
}
