import { RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import { useInstanceResourcesQueries } from "@/features/resources/queries"
import { ServiceResourceDetailPane } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"

export function ResourceDetailView(props: {
  instanceId: string
  resourceUri: string
  services: ServiceInstance[]
  onBack: () => void
}) {
  const { t } = useI18n()
  const instance = props.services.find((service) => service.instance_id === props.instanceId)
  const resourceQueries = useInstanceResourcesQueries(instance ? [instance] : [])
  const resources = resourceQueries[0]?.data || []
  const resource = resources.find((item) => item.uri === props.resourceUri) ?? null
  const loading = Boolean(instance) && resourceQueries.some((query) => query.isFetching)
  const error = resourceQueries.find((query) => query.error)?.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("resourcesFailedToLoad")

  async function loadResources() {
    const results = await Promise.all(resourceQueries.map((query) => query.refetch()))
    const failed = results.find((result) => result.error)
    if (failed?.error) {
      toast.error(failed.error instanceof Error ? failed.error.message : String(failed.error))
    }
  }

  const resourceLabel = resource?.title?.trim() || resource?.name || props.resourceUri
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
          ariaLabel={t("resource")}
          parentLabel={t("resourceRegistryTitle")}
          onParentClick={props.onBack}
          currentLabel={resourceLabel}
        />
        <PanelCard className="h-full min-h-0 flex-1">
          <PageEmpty
            title={t("noResourceSelected")}
            description={t("noResourceSelectedDescription")}
            onRefresh={props.onBack}
          />
        </PanelCard>
      </>
    )
  }

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("resource")}
        parentLabel={t("resourceRegistryTitle")}
        onParentClick={props.onBack}
        currentLabel={resourceLabel}
        subtitle={headerSubtitle}
      />
      <PanelCard
        variant="plain"
        className="flex h-full min-h-0 flex-1 flex-col gap-4 overflow-hidden"
      >
        <div className="flex flex-wrap items-center justify-end gap-2 border-b pb-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => void loadResources()}
            disabled={loading}
          >
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </div>

        {error ? (
          <PageError
            title={t("resourcesFailedToLoad")}
            message={errorMessage}
            onRefresh={loadResources}
          />
        ) : loading && !resource ? (
          <PageSkeleton />
        ) : resource ? (
          <ScrollPane className="min-h-0 flex-1">
            <ServiceResourceDetailPane resource={resource} service={instance} />
          </ScrollPane>
        ) : (
          <PageEmpty
            title={t("noResourcesFound")}
            description={t("noResourcesScopeDescription")}
            onRefresh={loadResources}
          />
        )}
      </PanelCard>
    </>
  )
}
