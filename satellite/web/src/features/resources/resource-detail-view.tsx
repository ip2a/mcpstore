import { useEffect, useState } from "react"
import { ClipboardIcon, FileIcon, PanelRightCloseIcon, PanelRightIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  ResourceDetailDocBody,
  ResourceDetailDocHeader,
  ResourcePlaygroundAside,
} from "@/components/shared/resource-detail-playground"
import { breadcrumbUppercase, WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { toolDetailPlaygroundGrid } from "@/components/shared/tool-detail-section-layout"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useInstanceResourcesQueries } from "@/features/resources/queries"
import { readInstanceResource, type ResourceInfo, type ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

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

  const [playgroundOpen, setPlaygroundOpen] = useState(true)
  const [content, setContent] = useState<unknown>(null)
  const [reading, setReading] = useState(false)

  useEffect(() => {
    setContent(null)
  }, [props.resourceUri, props.instanceId])

  async function loadResources() {
    const results = await Promise.all(resourceQueries.map((query) => query.refetch()))
    const failed = results.find((result) => result.error)
    if (failed?.error) {
      toast.error(failed.error instanceof Error ? failed.error.message : String(failed.error))
    }
  }

  async function onReadResource() {
    if (!instance || !resource) return
    setReading(true)
    try {
      const result = await readInstanceResource(instance, resource.uri)
      setContent(result)
    } catch (err) {
      const message = err instanceof Error ? err.message : t("readResourceFailed")
      toast.error(message)
    } finally {
      setReading(false)
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
          <div className="flex shrink-0 flex-wrap justify-end gap-2">
            {resource ? (
              <Button size="sm" onClick={() => void onReadResource()} disabled={reading}>
                {reading ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <FileIcon data-icon="inline-start" />
                )}
                {reading ? t("reading") : t("read")}
              </Button>
            ) : null}
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
            {resource ? (
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPlaygroundOpen((open) => !open)}
                aria-label={playgroundOpen ? t("collapse") : t("expand")}
                title={playgroundOpen ? t("collapse") : t("expand")}
              >
                {playgroundOpen ? <PanelRightCloseIcon /> : <PanelRightIcon />}
              </Button>
            ) : null}
          </div>
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
          <div
            className={cn(toolDetailPlaygroundGrid, !playgroundOpen && "@min-[42rem]:grid-cols-1")}
          >
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ScrollPane className="min-h-0 flex-1">
                <ResourceDetailDocHeader resource={resource} serviceState={instance.state} />
                <ResourceDetailDocBody
                  resource={resource}
                  serviceName={instance.service_name}
                  scope={instance.scope}
                />
              </ScrollPane>
            </div>
            {playgroundOpen ? (
              <ResourcePlaygroundAside
                resource={resource}
                service={instance}
                onRead={() => void onReadResource()}
                reading={reading}
                content={content}
              />
            ) : null}
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
