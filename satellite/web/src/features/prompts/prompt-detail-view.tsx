import { useEffect, useState } from "react"
import { ClipboardIcon, PanelRightCloseIcon, PanelRightIcon, ScrollTextIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  PromptDetailDocBody,
  PromptDetailDocHeader,
  PromptPlaygroundAside,
} from "@/components/shared/prompt-detail-playground"
import { breadcrumbUppercase, WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { toolDetailPlaygroundGrid } from "@/components/shared/tool-detail-section-layout"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useInstancePromptsQueries } from "@/features/prompts/queries"
import { usePromptArgsForm } from "@/features/prompts/use-prompt-args-form"
import { getInstancePrompt, type PromptInfo, type ServiceInstance } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

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

  const { values: promptArgs, setField: setPromptArg } = usePromptArgsForm(prompt)
  const [playgroundOpen, setPlaygroundOpen] = useState(true)
  const [content, setContent] = useState<unknown>(null)
  const [getting, setGetting] = useState(false)

  useEffect(() => {
    setContent(null)
  }, [props.promptName, props.instanceId])

  async function loadPrompts() {
    const results = await Promise.all(promptQueries.map((query) => query.refetch()))
    const failed = results.find((result) => result.error)
    if (failed?.error) {
      toast.error(failed.error instanceof Error ? failed.error.message : String(failed.error))
    }
  }

  async function onGetPrompt() {
    if (!instance || !prompt) return
    setGetting(true)
    try {
      const result = await getInstancePrompt(instance, prompt.name, promptArgs)
      setContent(result)
    } catch (err) {
      const message = err instanceof Error ? err.message : t("getPromptFailed")
      toast.error(message)
    } finally {
      setGetting(false)
    }
  }

  const promptLabel = prompt?.title?.trim() || prompt?.name || props.promptName
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
          ariaLabel={t("promptLabel")}
          crumbs={[{ label: t("navPrompts"), onClick: props.onBack }, { label: promptLabel }]}
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
        crumbs={[
          { label: t("navPrompts"), onClick: props.onBack },
          { label: scopeCrumbLabel!, onClick: props.onBack },
          {
            label: breadcrumbUppercase(instance.service_name),
            onClick: props.onBack,
          },
          { label: promptLabel },
        ]}
        subtitle={headerSubtitle}
      />
      <PanelCard
        variant="plain"
        className="flex h-full min-h-0 flex-1 flex-col gap-4 overflow-hidden"
      >
        <div className="flex flex-wrap items-center justify-end gap-2 border-b pb-2">
          <div className="flex shrink-0 flex-wrap justify-end gap-2">
            {prompt ? (
              <Button size="sm" onClick={() => void onGetPrompt()} disabled={getting}>
                {getting ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <ScrollTextIcon data-icon="inline-start" />
                )}
                {getting ? t("executing") : t("get")}
              </Button>
            ) : null}
            {prompt ? (
              <Button size="sm" variant="outline" onClick={() => void copyPrompt(prompt)}>
                <ClipboardIcon data-icon="inline-start" />
                {t("copy")}
              </Button>
            ) : null}
            <Button
              size="sm"
              variant="outline"
              onClick={() => void loadPrompts()}
              disabled={loading}
            >
              <RefreshCwSpinIcon data-icon="inline-start" spinning={loading} />
              {t("refresh")}
            </Button>
            {prompt ? (
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
            title={t("promptsFailedToLoad")}
            message={errorMessage}
            onRefresh={loadPrompts}
          />
        ) : loading && !prompt ? (
          <PageSkeleton />
        ) : prompt ? (
          <div
            className={cn(toolDetailPlaygroundGrid, !playgroundOpen && "@min-[42rem]:grid-cols-1")}
          >
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ScrollPane className="min-h-0 flex-1">
                <PromptDetailDocHeader prompt={prompt} serviceState={instance.state} />
                <PromptDetailDocBody
                  prompt={prompt}
                  promptArgs={promptArgs}
                  onPromptArgChange={setPromptArg}
                  serviceName={instance.service_name}
                  scope={instance.scope}
                />
              </ScrollPane>
            </div>
            {playgroundOpen ? (
              <PromptPlaygroundAside
                prompt={prompt}
                service={instance}
                promptArgs={promptArgs}
                onGet={() => void onGetPrompt()}
                getting={getting}
                content={content}
              />
            ) : null}
          </div>
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

async function copyPrompt(prompt: PromptInfo) {
  await navigator.clipboard.writeText(JSON.stringify(prompt, null, 2))
  toast.success("Prompt copied")
}
