import { useMemo, useState } from "react"
import { PlayIcon } from "lucide-react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import {
  toolDetailPlaygroundAside,
} from "@/components/shared/tool-detail-section-layout"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockBody,
  CodeBlockCopyButton,
  CodeBlockFooter,
  CodeBlockHeader,
  CodeBlockJson,
  CodeBlockMethod,
  CodeBlockString,
  CodeBlockTitle,
} from "@/components/ui/code-block"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { TypographyH2, TypographyLead } from "@/components/ui/typography"
import { OverrideEditor } from "@/features/overrides/override-editor"
import {
  readInstanceResource,
  type ResourceInfo,
  type ScopeRef,
  type ServiceInstance,
  type ServiceState,
} from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { hasJsonContent } from "@/lib/tool-info"
import { cn } from "@/lib/utils"

function resourceMimeType(resource: ResourceInfo) {
  return String(resource.mimeType || "").trim()
}

export function buildResourceCliCommand(input: { instanceId: string; uri: string }) {
  return [
    "mcpstore resources read \\",
    `  --instance "${input.instanceId}" \\`,
    `  --uri "${input.uri}"`,
  ].join("\n")
}

export function ResourceDetailDocHeader({
  resource,
  serviceState,
  className,
}: {
  resource: ResourceInfo
  serviceState?: ServiceState | null
  className?: string
}) {
  const title = resource.title?.trim() || resource.name || resource.uri

  return (
    <header className={cn("shrink-0 space-y-3 border-b pb-4", className)}>
      <div className="flex min-w-0 items-center justify-between gap-3">
        <TypographyH2 className="min-w-0 border-b-0 pb-0 break-words font-mono text-xl">
          {title}
        </TypographyH2>
        {serviceState ? <ServiceStatusBadge state={serviceState} /> : null}
      </div>
      <ToolDescriptionBlock
        description={resource.description}
        showLabel={false}
        omitStructuredSections
        className="text-foreground"
      />
    </header>
  )
}

export function ResourceDetailDocBody({
  resource,
  serviceName,
  scope,
  className,
}: {
  resource: ResourceInfo
  serviceName?: string
  scope?: ScopeRef
  className?: string
}) {
  const { t } = useI18n()
  const mimeType = resourceMimeType(resource)
  const annotations = resource.annotations ?? null
  const meta = resource._meta ?? null
  const fields: Array<{ label: string; value: string; mono?: boolean }> = [
    { label: t("uri"), value: resource.uri, mono: true },
  ]
  if (resource.name) fields.push({ label: t("name"), value: resource.name })
  if (resource.title?.trim() && resource.title.trim() !== resource.name) {
    fields.push({ label: t("titleLabel"), value: resource.title.trim() })
  }
  if (mimeType) fields.push({ label: t("mimeType"), value: mimeType, mono: true })

  return (
    <div className={cn("min-w-0 space-y-6 pb-1 pt-4", className)}>
      <section>
        <TypographyH2>{t("resource")}</TypographyH2>
        <dl className="mt-3 grid gap-4">
          {fields.map((field) => (
            <div key={field.label} className="grid gap-1 border-b pb-4 last:border-b-0 last:pb-0">
              <dt className="font-mono text-xs uppercase text-muted-foreground">{field.label}</dt>
              <dd className={cn("break-all text-sm", field.mono && "font-mono")}>{field.value}</dd>
            </div>
          ))}
        </dl>
      </section>

      {hasJsonContent(annotations) ? (
        <section>
          <TypographyH2>{t("annotations")}</TypographyH2>
          <div className="mt-3">
            <JsonBlock value={annotations} />
          </div>
        </section>
      ) : null}

      {hasJsonContent(meta) ? (
        <section>
          <TypographyH2>{t("meta")}</TypographyH2>
          <div className="mt-3">
            <JsonBlock value={meta} />
          </div>
        </section>
      ) : null}

      {serviceName && scope ? (
        <OverrideEditor
          kind="resource"
          serviceName={serviceName}
          componentKey={resource.uri}
          scope={scope}
        />
      ) : null}
    </div>
  )
}

export function ResourcePlaygroundAside({
  resource,
  service,
  onRead,
  reading,
  content,
  className,
}: {
  resource: ResourceInfo
  service: ServiceInstance
  onRead?: () => void
  reading?: boolean
  content?: unknown
  className?: string
}) {
  const { t } = useI18n()
  const [localContent, setLocalContent] = useState<unknown>(null)
  const [localReading, setLocalReading] = useState(false)

  const controlled = onRead !== undefined
  const isReading = controlled ? Boolean(reading) : localReading
  const displayContent = controlled ? content : localContent

  const cliCommand = useMemo(
    () =>
      buildResourceCliCommand({
        instanceId: service.instance_id,
        uri: resource.uri,
      }),
    [resource.uri, service.instance_id],
  )

  const responseText = useMemo(
    () =>
      displayContent == null
        ? JSON.stringify({ contents: [{ uri: resource.uri, text: "..." }] }, null, 2)
        : JSON.stringify(displayContent, null, 2),
    [displayContent, resource.uri],
  )

  async function requestRead() {
    if (controlled) {
      onRead?.()
      return
    }
    setLocalReading(true)
    try {
      const result = await readInstanceResource(service, resource.uri)
      setLocalContent(result)
    } catch (err) {
      const message = err instanceof Error ? err.message : t("readResourceFailed")
      toast.error(message)
    } finally {
      setLocalReading(false)
    }
  }

  return (
    <aside
      className={cn(
        toolDetailPlaygroundAside,
        "h-full min-h-0 min-w-0 w-full flex-col gap-4",
        className,
      )}
    >
      <CodeBlock variant="request" className="flex shrink-0 flex-col">
        <CodeBlockHeader variant="request">
          <CodeBlockTitle>
            <CodeBlockMethod className="bg-sky-500/15 text-sky-300">READ</CodeBlockMethod>
            <span className="truncate text-zinc-300">{resource.uri}</span>
          </CodeBlockTitle>
          <CodeBlockActions>
            <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">
              {t("cliRequestLabel")}
            </span>
            <CodeBlockCopyButton
              value={cliCommand}
              className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50"
            />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="request" maxHeight="11rem">
          <CodeBlockString lines={cliCommand.split("\n")} />
        </CodeBlockBody>
        <CodeBlockFooter variant="request" className="justify-end">
          <Button
            size="sm"
            className="h-8 bg-zinc-100 text-zinc-900 hover:bg-white"
            onClick={() => void requestRead()}
            disabled={isReading}
          >
            {isReading ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <PlayIcon data-icon="inline-start" />
            )}
            {isReading ? t("reading") : t("testRequest")}
          </Button>
        </CodeBlockFooter>
      </CodeBlock>

      <CodeBlock variant="response" className="flex min-h-0 flex-1 flex-col">
        <CodeBlockHeader variant="response" className="shrink-0">
          <CodeBlockTitle>
            <span className="font-semibold text-foreground">
              {displayContent == null ? t("dynamicValue") : "200"}
            </span>
          </CodeBlockTitle>
          <CodeBlockActions>
            <CodeBlockCopyButton value={responseText} />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="response" fill nativeScroll>
          <CodeBlockJson text={responseText} />
        </CodeBlockBody>
        <CodeBlockFooter variant="response" className="shrink-0">
          <TypographyLead className="m-0 text-xs text-muted-foreground">
            {displayContent == null ? t("requestPreview") : t("responseOk")}
          </TypographyLead>
        </CodeBlockFooter>
      </CodeBlock>
    </aside>
  )
}
