import { EntityRow } from "@/components/shared/entity-row"
import { ToolSchemaEmptyHint } from "@/components/shared/tool-schema-empty-hint"
import { Badge } from "@/components/ui/badge"
import { useI18n } from "@/lib/i18n-context"
import { formatSchemaDefaultValue, getSchemaFieldSubtitle, resolveSchemaFieldType } from "@/lib/tool-info"

type SchemaField = Record<string, unknown>

type ToolSchemaFieldsProps = {
  fields: Array<[string, SchemaField]>
  required?: string[]
  emptyMessage: string
  search?: string
}

const paramBadgeClass = "px-2.5 py-1 text-sm"

export function filterSchemaFields(
  fields: Array<[string, SchemaField]>,
  search: string,
): Array<[string, SchemaField]> {
  const normalizedSearch = search.trim().toLowerCase()
  if (!normalizedSearch) return fields

  return fields.filter(([name, field]) => {
    const title = typeof field.title === "string" ? field.title : ""
    const fieldType = resolveSchemaFieldType(field)
    return [name, title, fieldType].some((value) => value.toLowerCase().includes(normalizedSearch))
  })
}

export function ToolSchemaFields({ emptyMessage, fields, required = [], search = "" }: ToolSchemaFieldsProps) {
  const { t } = useI18n()
  const filteredFields = filterSchemaFields(fields, search)

  if (!filteredFields.length) {
    return (
      <ToolSchemaEmptyHint>
        {search.trim() ? t("noParamsMatchSearch") : emptyMessage}
      </ToolSchemaEmptyHint>
    )
  }

  return (
    <div className="flex flex-col">
      {filteredFields.map(([name, field], index) => {
        const isRequired = required.includes(name)
        const fieldType = resolveSchemaFieldType(field)
        const defaultValue = field.default
        const subtitle = getSchemaFieldSubtitle(field)
        const isLast = index === filteredFields.length - 1

        return (
          <EntityRow
            key={name}
            variant="inline"
            className={isLast ? "border-b-0" : undefined}
            actions={
              <div className="flex flex-wrap gap-2">
                <Badge variant="outline" className={paramBadgeClass}>{fieldType}</Badge>
                {isRequired ? (
                  <Badge variant="secondary" className={paramBadgeClass}>{t("required")}</Badge>
                ) : (
                  <Badge variant="outline" className={paramBadgeClass}>{t("optional")}</Badge>
                )}
                {defaultValue !== undefined ? (
                  <Badge variant="outline" className={`font-mono ${paramBadgeClass}`}>
                    {t("defaultValue")} {formatSchemaDefaultValue(defaultValue)}
                  </Badge>
                ) : null}
              </div>
            }
          >
            <div className="min-w-0">
              <code className="text-sm font-semibold">{name}</code>
              {subtitle ? <p className="mt-0.5 text-sm text-muted-foreground">{subtitle}</p> : null}
            </div>
          </EntityRow>
        )
      })}
    </div>
  )
}
