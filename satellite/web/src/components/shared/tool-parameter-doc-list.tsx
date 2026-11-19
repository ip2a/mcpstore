import { ToolArgFieldInput } from "@/components/shared/tool-args-form"
import { ToolSchemaEmptyHint } from "@/components/shared/tool-schema-empty-hint"
import { TypographyInlineCode } from "@/components/ui/typography"
import { InlineRouteField } from "@/features/routes/inline-route-field"
import { useToolInlineRoute } from "@/features/routes/use-tool-inline-route"
import { useI18n } from "@/lib/i18n-context"
import type { ScopeRef } from "@/lib/api"
import { resolveFormFieldType } from "@/lib/tool-args"
import {
  formatSchemaDefaultValue,
  resolveParameterDoc,
  resolveSchemaFieldType,
} from "@/lib/tool-info"
import { cn } from "@/lib/utils"

type SchemaField = Record<string, unknown>

type ToolParameterDocListProps = {
  fields: Array<[string, SchemaField]>
  required?: string[]
  emptyMessage: string
  className?: string
  values?: Record<string, unknown>
  onChange?: (name: string, value: unknown) => void
  /** `name: text` description parsed from the Args/Parameters section; a source parallel to the schema description. */
  proseParamDocs?: Record<string, string>
  serviceName?: string
  scope?: ScopeRef
  originalToolName?: string
}

export function ToolParameterDocList({
  className,
  emptyMessage,
  fields,
  onChange,
  proseParamDocs = {},
  required = [],
  values,
  serviceName,
  scope,
  originalToolName,
}: ToolParameterDocListProps) {
  const { t } = useI18n()
  const editable = Boolean(values && onChange)
  const route = useToolInlineRoute(serviceName, originalToolName, scope)

  if (!fields.length) {
    return <ToolSchemaEmptyHint>{emptyMessage}</ToolSchemaEmptyHint>
  }

  return (
    <div className={cn("@container flex flex-col", className)}>
      {fields.map(([name, field], index) => {
        const isRequired = required.includes(name)
        const fieldType = resolveSchemaFieldType(field)
        const formType = resolveFormFieldType(field)
        const argMeta = route.enabled ? route.argMeta(name) : null
        const paramDoc = resolveParameterDoc(
          name,
          field,
          proseParamDocs,
          argMeta?.originalName,
        )
        const defaultValue = field.default
        const isLast = index === fields.length - 1

        return (
          <article
            key={name}
            className={cn("grid gap-3 border-b py-4", isLast && "border-b-0 pb-0")}
          >
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              {route.enabled ? (
                <InlineRouteField
                  value={name}
                  originalValue={argMeta?.originalName || name}
                  overridden={Boolean(argMeta?.nameOverridden)}
                  label={t("argNewName")}
                  busy={route.busy}
                  textClassName="break-all font-mono text-sm font-semibold"
                  onSave={(next) => route.saveArgName(name, next)}
                />
              ) : (
                <TypographyInlineCode className="break-all text-sm font-semibold">
                  {name}
                </TypographyInlineCode>
              )}
              <span className="font-mono text-xs text-muted-foreground">{fieldType}</span>
              {isRequired ? (
                <span className="text-xs font-medium text-foreground">{t("required")}</span>
              ) : (
                <span className="text-xs text-muted-foreground">{t("optional")}</span>
              )}
              {defaultValue !== undefined ? (
                <span className="text-xs text-muted-foreground">
                  {t("defaultValue")}{" "}
                  <span className="border-b border-dotted border-muted-foreground/60 font-mono">
                    {formatSchemaDefaultValue(defaultValue)}
                  </span>
                </span>
              ) : null}
            </div>
            {paramDoc || route.enabled ? (
              route.enabled ? (
                <InlineRouteField
                  value={paramDoc || ""}
                  originalValue={
                    argMeta?.descriptionOverridden
                      ? proseParamDocs[argMeta.originalName]?.trim() || ""
                      : paramDoc || ""
                  }
                  overridden={Boolean(argMeta?.descriptionOverridden)}
                  label={t("routeDescription")}
                  multiline
                  busy={route.busy}
                  className="w-full"
                  textClassName={cn(
                    "text-sm leading-relaxed",
                    !argMeta?.descriptionOverridden && "text-muted-foreground",
                  )}
                  onSave={(next) => route.saveArgDescription(name, next)}
                >
                  {paramDoc || (
                    <span className="text-muted-foreground/70 italic">{t("noDescription")}</span>
                  )}
                </InlineRouteField>
              ) : paramDoc ? (
                <p className="text-sm leading-relaxed text-muted-foreground">{paramDoc}</p>
              ) : null
            ) : null}
            {editable ? (
              <div
                className={cn(
                  "grid gap-3 @min-[32rem]:grid-cols-[minmax(0,3fr)_minmax(0,7fr)]",
                  formType === "boolean" ? "@min-[32rem]:items-center" : "@min-[32rem]:items-start",
                )}
              >
                <span aria-hidden="true" />
                <ToolArgFieldInput
                  id={`tool-param-${name}`}
                  field={field}
                  type={formType}
                  value={values![name]}
                  valueAlign={formType === "boolean" ? "right" : "left"}
                  compact
                  onChange={(value) => onChange!(name, value)}
                />
              </div>
            ) : null}
          </article>
        )
      })}
    </div>
  )
}
