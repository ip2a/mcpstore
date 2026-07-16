import { JsonBlock } from "@/components/shared/json-block"
import { Badge } from "@/components/ui/badge"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type ContentBlock = {
  type: string
  text?: string
  data?: string
  mimeType?: string
  uri?: string
  name?: string
  resource?: {
    uri?: string
    mimeType?: string
    text?: string
    blob?: string
  }
}

function normalizeToolResult(result: unknown) {
  if (result == null) {
    return { content: [] as ContentBlock[], structuredContent: undefined, isError: false }
  }

  if (typeof result === "string") {
    return { content: [{ type: "text", text: result }], structuredContent: undefined, isError: false }
  }

  if (typeof result !== "object" || Array.isArray(result)) {
    return { content: [{ type: "text", text: String(result) }], structuredContent: undefined, isError: false }
  }

  const record = result as Record<string, unknown>
  const content = Array.isArray(record.content) ? (record.content as ContentBlock[]) : []

  return {
    content,
    structuredContent: record.structuredContent,
    isError: Boolean(record.isError),
  }
}

function TextBlock({ text, isError }: { text: string; isError?: boolean }) {
  return (
    <pre
      className={cn(
        "overflow-x-auto whitespace-pre-wrap break-words rounded-lg border px-4 py-3 font-mono text-sm leading-relaxed",
        isError ? "border-destructive/40 bg-destructive/10 text-destructive" : "border-border/60 bg-muted/20 text-foreground",
      )}
    >
      {text}
    </pre>
  )
}

function ImageBlock({ data, mimeType }: { data: string; mimeType?: string }) {
  const type = mimeType || "image/png"
  const src = data.startsWith("data:") ? data : `data:${type};base64,${data}`

  return (
    <div className="overflow-hidden rounded-lg border border-border/60 bg-muted/10 p-3">
      <img src={src} alt="" className="mx-auto max-h-[min(50dvh,480px)] max-w-full object-contain" />
    </div>
  )
}

function ResourceBlock({ block }: { block: ContentBlock }) {
  const { t } = useI18n()
  const resource = block.resource
  const uri = resource?.uri || block.uri
  const mimeType = resource?.mimeType || block.mimeType
  const text = resource?.text || block.text

  return (
    <div className="space-y-2 rounded-lg border border-border/60 bg-muted/10 p-4">
      {uri ? (
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className="text-muted-foreground">{t("uri")}</span>
          <code className="break-all font-mono text-xs">{uri}</code>
        </div>
      ) : null}
      {mimeType ? (
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className="text-muted-foreground">{t("mimeType")}</span>
          <Badge variant="outline">{mimeType}</Badge>
        </div>
      ) : null}
      {text ? <TextBlock text={text} /> : null}
    </div>
  )
}

function ContentBlockView({ block, isError }: { block: ContentBlock; isError?: boolean }) {
  switch (block.type) {
    case "text":
      return <TextBlock text={block.text || ""} isError={isError} />
    case "image":
      return block.data ? <ImageBlock data={block.data} mimeType={block.mimeType} /> : null
    case "resource":
    case "resource_link":
      return <ResourceBlock block={block} />
    default:
      return <JsonBlock value={block} className="max-h-64" />
  }
}

export function ToolResultView({ result, className }: { result: unknown; className?: string }) {
  const { t } = useI18n()
  const { content, structuredContent, isError } = normalizeToolResult(result)

  if (!content.length && structuredContent == null) {
    return (
      <p className={cn("rounded-lg border border-dashed bg-muted/10 px-4 py-6 text-sm text-muted-foreground", className)}>
        {t("toolResultEmpty")}
      </p>
    )
  }

  return (
    <div className={cn("flex flex-col gap-4", className)}>
      {isError ? (
        <Badge variant="destructive" className="w-fit">
          {t("toolResultError")}
        </Badge>
      ) : null}
      {content.map((block, index) => (
        <ContentBlockView key={`${block.type}-${index}`} block={block} isError={isError} />
      ))}
      {structuredContent != null ? (
        <section className="space-y-2">
          <p className="text-sm font-medium text-muted-foreground">{t("structuredContent")}</p>
          <JsonBlock value={structuredContent} />
        </section>
      ) : null}
    </div>
  )
}
