import { JsonBlock } from "@/components/shared/json-block"
import { SectionHeading } from "@/components/shared/section-heading"
import { ToolSchemaEmptyHint } from "@/components/shared/tool-schema-empty-hint"
import {
  toolDetailSectionAside,
  toolDetailSectionGrid,
  toolDetailSectionLabel,
  toolDetailSplitSection,
} from "@/components/shared/tool-detail-section-layout"
import { ToolSchemaFields } from "@/components/shared/tool-schema-fields"
import { useI18n } from "@/lib/i18n-context"
import type { ToolInfo } from "@/lib/api"
import { getToolAnnotations, getToolMeta, getToolOutputSchema, getToolSchema, hasJsonContent } from "@/lib/tool-info"

type SchemaField = Record<string, unknown>
type JsonSchema = { properties?: Record<string, SchemaField>; required?: string[] }

function schemaFields(schema: JsonSchema) {
  return Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))
}

export function ToolInputSchemaSection({ tool }: { tool: ToolInfo }) {
  const { t } = useI18n()
  const inputSchema = getToolSchema(tool) as JsonSchema
  const inputParams = schemaFields(inputSchema)
  const hasInputSchema = inputParams.length > 0

  return (
    <section className="border-b pb-4">
      <SectionHeading title={t("inputParams")} titleAs="h2" className="border-b-0 pb-3" />
      {hasInputSchema ? (
        <ToolSchemaFields
          fields={inputParams}
          required={inputSchema.required}
          emptyMessage={t("noInputSchema")}
        />
      ) : (
        <ToolSchemaEmptyHint>{t("noInputSchema")}</ToolSchemaEmptyHint>
      )}
    </section>
  )
}

export function ToolOutputSchemaSection({
  tool,
  layout = "stack",
}: {
  tool: ToolInfo
  layout?: "stack" | "split"
}) {
  const { t } = useI18n()
  const outputSchema = getToolOutputSchema(tool) as JsonSchema
  const outputParams = schemaFields(outputSchema)
  const hasOutputSchema = outputParams.length > 0
  const content = hasOutputSchema ? (
    <ToolSchemaFields
      fields={outputParams}
      required={outputSchema.required}
      emptyMessage={t("noOutputSchema")}
    />
  ) : (
    <ToolSchemaEmptyHint align={layout === "split" ? "right" : "left"}>
      {t("noOutputSchema")}
    </ToolSchemaEmptyHint>
  )

  if (layout === "split") {
    return (
      <section className={toolDetailSplitSection}>
        <div className={toolDetailSectionGrid}>
          <div className={toolDetailSectionAside}>
            <h2 className={toolDetailSectionLabel}>{t("outputParams")}</h2>
          </div>
          {content}
        </div>
      </section>
    )
  }

  return (
    <section className="border-b pb-4">
      <SectionHeading title={t("outputParams")} titleAs="h2" className="border-b-0 pb-3" />
      {content}
    </section>
  )
}

export function ToolAnnotationsSection({ tool }: { tool: ToolInfo }) {
  const { t } = useI18n()
  const annotations = getToolAnnotations(tool)
  if (!hasJsonContent(annotations)) return null

  return (
    <section className="border-b pb-4">
      <SectionHeading title={t("annotations")} titleAs="h2" className="border-b-0 pb-3" />
      <JsonBlock value={annotations} />
    </section>
  )
}

export function ToolMetaSection({ tool }: { tool: ToolInfo }) {
  const { t } = useI18n()
  const meta = getToolMeta(tool)
  if (!hasJsonContent(meta)) return null

  return (
    <section className="border-b pb-4">
      <SectionHeading title={t("meta")} titleAs="h2" className="border-b-0 pb-3" />
      <JsonBlock value={meta} />
    </section>
  )
}
