import { ClipboardIcon, EyeIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { EntityRow } from "@/components/shared/entity-row"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { useI18n } from "@/lib/i18n-context"
import type { ToolInfo } from "@/lib/api"
import { getToolSchema } from "@/lib/tool-info"

export function ToolCard({
  tool,
  sourceLabel,
  onRun,
  onDetail,
  showActions = true,
  variant = "card",
}: {
  tool: ToolInfo
  sourceLabel?: string
  onRun?: () => void
  onDetail?: () => void
  showActions?: boolean
  variant?: "card" | "plain"
}) {
  const { t } = useI18n()
  const schema = getToolSchema(tool) as { properties?: Record<string, { type?: string; description?: string }>; required?: string[] }
  const params = Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))

  async function onCopy() {
    await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
    toast.success(t("toolCopied"))
  }

  if (variant === "plain") {
    return (
      <div className="border-b pb-4">
        <ToolDescriptionBlock description={tool.description} />
      </div>
    )
  }

  const content = (
    <>
      <SectionHeading
        title={tool.name}
        titleAs="h2"
        description={tool.description || t("noDescription")}
        className="border-b-0 pb-0"
        actions={
          showActions ? (
            <div className="flex flex-wrap justify-end gap-2">
              <Button size="sm" onClick={onRun}>
                <WrenchIcon data-icon="inline-start" />
                {t("run")}
              </Button>
              <Button size="sm" variant="outline" onClick={onDetail}>
                <EyeIcon data-icon="inline-start" />
                {t("details")}
              </Button>
              <Button size="sm" variant="outline" onClick={onCopy}>
                <ClipboardIcon data-icon="inline-start" />
                {t("copy")}
              </Button>
            </div>
          ) : undefined
        }
      />
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap gap-2">
          {sourceLabel ? <Badge variant="secondary">{sourceLabel}</Badge> : null}
          {schema.required?.length ? (
            <Badge variant="outline">{t("paramCount", { count: schema.required.length })} {t("required")}</Badge>
          ) : (
            <Badge variant="outline">{t("optional")}</Badge>
          )}
        </div>
        {params.length ? (
          params.slice(0, 4).map(([name, meta]) => (
            <EntityRow key={name} actions={<Badge variant="outline">{meta.type || "any"}</Badge>}>
              <div className="min-w-0">
                <code className="text-sm font-medium">{name}</code>
                <p className="truncate text-sm text-muted-foreground">{meta.description || t("noDescription")}</p>
              </div>
            </EntityRow>
          ))
        ) : (
          <p className="text-sm text-muted-foreground">{t("toolNoArgsRunDirectly")}</p>
        )}
      </div>
    </>
  )

  return <PanelCard>{content}</PanelCard>
}
