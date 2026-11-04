import { RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import { useInstancePromptsQueries } from "@/features/prompts/queries"
import { ServicePromptDetailPane } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"

export function PromptDetailView(props: {
  instanceId: string
  promptName: string
  services: ServiceInstance[]
  onBack: () => void
}) {
  const { t } = useI18n()
  const instance = props.services.find((service) => service.instance_id === props.instanceId)
  const promptQueries = useInstancePromptsQueries(instance ? [instance] : [])
  const prompts = promptQueries[0]?.data || []
  const prompt = prompts.find((item) => item.name === props.promptName) ?? null
  const loading = Boolean(instance) && promptQueries.some((query) => query.isFetching)
  const error = promptQueries.find((query) => query.error)?.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("promptsFailedToLoad")

  async function loadPrompts() {
    const results = await Promise.all(promptQueries.map((query) => query.refetch()))
    const failed = results.find((result) => result.error)
    if (failed?.error) {
      toast.error(failed.error instanceof Error ? failed.error.message : String(failed.error))
    }
  }

  const promptLabel = prompt?.title?.trim() || prompt?.name || props.promptName
  const scopeLabel = instance
    ? instance.scope.type === "store"
      ? t("storeScope")
      : t("agentScopeLabel", { agentId: instance.scope.agent_id || "-" })
    : undefined
  const headerSubtitle = instance
    ? [instance.service_name, scopeLabel].filter(Boolean).join(" · ")
    : undefined

  if (!instance) {
    return (
      <>
        <WorkspaceBreadcrumb
          ariaLabel={t("promptLabel")}
          parentLabel={t("promptRegistryTitle")}
          onParentClick={props.onBack}
          currentLabel={promptLabel}
        />
        <PanelCard className="h-full min-h-0 flex-1">
          <PageEmpty
            title={t("noPromptSelected")}
            description={t("noPromptSelectedDescription")}
            onRefresh={props.onBack}
          />
        </PanelCard>
      </>
    )
  }

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("promptLabel")}
        parentLabel={t("promptRegistryTitle")}
        onParentClick={props.onBack}
        currentLabel={promptLabel}
        subtitle={headerSubtitle}
      />
      <PanelCard
        variant="plain"
        className="flex h-full min-h-0 flex-1 flex-col gap-4 overflow-hidden"
      >
        <div className="flex flex-wrap items-center justify-end gap-2 border-b pb-2">
          <Button size="sm" variant="outline" onClick={() => void loadPrompts()} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </div>

        {error ? (
          <PageError
            title={t("promptsFailedToLoad")}
            message={errorMessage}
            onRefresh={loadPrompts}
          />
        ) : loading && !prompt ? (
          <PageSkeleton />
        ) : prompt ? (
          <ScrollPane className="min-h-0 flex-1">
            <ServicePromptDetailPane prompt={prompt} service={instance} />
          </ScrollPane>
        ) : (
          <PageEmpty
            title={t("noPromptsFound")}
            description={t("noPromptsScopeDescription")}
            onRefresh={loadPrompts}
          />
        )}
      </PanelCard>
    </>
  )
}
