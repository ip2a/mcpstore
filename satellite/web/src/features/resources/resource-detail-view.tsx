import { ClipboardIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { breadcrumbUppercase, WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Button } from "@/components/ui/button"
import { useInstanceResourcesQueries } from "@/features/resources/queries"
import { ServiceResourceDetailPane } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ResourceInfo, ServiceInstance } from "@/lib/api"

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
  const scopeCrumbLabel = instance
    ? breadcrumbUppercase(
        instance.scope.type === "store"
          ? t("store")
          : instance.scope.type === "agent"
            ? instance.scope.agent_id
            : t("filterAll"),
      )
    : undefined
  const headerSubtitle = instance
    ? [
        instance.service_name,
        instance.scope.type === "store"
          ? t("storeScope")
          : t("agentScopeLabel", { agentId: instance.scope.agent_id || "-" }),
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined

  if (!instance) {
    return (
      <>
        <WorkspaceBreadcrumb
          ariaLabel={t("resource")}
          crumbs={[{ label: t("navResources"), onClick: props.onBack }, { label: resourceLabel }]}
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
        crumbs={[
          { label: t("navResources"), onClick: props.onBack },
          { label: scopeCrumbLabel!, onClick: props.onBack },
          {
            label: breadcrumbUppercase(instance.service_name),
            onClick: props.onBack,
          },
          { label: resourceLabel },
        ]}
        subtitle={headerSubtitle}
      />
      <PanelCard
        variant="plain"
        className="flex h-full min-h-0 flex-1 flex-col gap-4 overflow-hidden"
      >
        <div className="flex flex-wrap items-center justify-end gap-2 border-b pb-2">
          {resource ? (
            <Button size="sm" variant="outline" onClick={() => void copyResource(resource)}>
              <ClipboardIcon data-icon="inline-start" />
              {t("copy")}
            </Button>
          ) : null}
          <Button
            size="sm"
            variant="outline"
            onClick={() => void loadResources()}
            disabled={loading}
          >
            <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
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
          <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
            <ScrollPane className="min-h-0 flex-1">
              <ServiceResourceDetailPane resource={resource} service={instance} />
            </ScrollPane>
          </div>
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

async function copyResource(resource: ResourceInfo) {
  await navigator.clipboard.writeText(JSON.stringify(resource, null, 2))
  toast.success("Resource copied")
}
