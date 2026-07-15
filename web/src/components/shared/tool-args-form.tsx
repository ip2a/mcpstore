import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { ToolSchemaEmptyHint } from "@/components/shared/tool-schema-empty-hint"
import { useI18n } from "@/lib/i18n-context"
import {
  formatDefaultPlaceholder,
  getSchemaFieldLabel,
  getSortedSchemaFields,
  isEmptyFormValue,
  isSchemaFieldRequired,
  resolveFormFieldType,
  type FormFieldType,
} from "@/lib/tool-args"
import { getSchemaFieldSubtitle } from "@/lib/tool-info"
import { cn } from "@/lib/utils"

type SchemaField = Record<string, unknown>
type ToolSchema = { properties?: Record<string, SchemaField>; required?: string[] }

const inputMutedClass = "bg-muted/50 text-foreground placeholder:text-muted-foreground"
const compactTextareaClass = "min-h-9 resize-y py-1 text-sm leading-normal"

export function ToolArgsForm({
  className,
  schema,
  values,
  onChange,
  valueAlign = "left",
}: {
  className?: string
  schema: ToolSchema
  values: Record<string, unknown>
  onChange: (name: string, value: unknown) => void
  valueAlign?: "left" | "right"
}) {
  const { t } = useI18n()
  const fields = getSortedSchemaFields(schema)

  if (!fields.length) {
    return (
      <ToolSchemaEmptyHint align={valueAlign}>
        {t("toolNoArgsRunDirectly")}
      </ToolSchemaEmptyHint>
    )
  }

  return (
    <div className={cn("flex flex-col", className)}>
      {fields.map(([name, field], index) => (
        <ToolArgRow
          key={name}
          name={name}
          field={field}
          required={isSchemaFieldRequired(name, schema)}
          value={values[name]}
          valueAlign={valueAlign}
          isLast={index === fields.length - 1}
          onChange={(value) => onChange(name, value)}
        />
      ))}
    </div>
  )
}

function ToolArgRow({
  name,
  field,
  required,
  value,
  valueAlign,
  isLast,
  onChange,
}: {
  name: string
  field: SchemaField
  required: boolean
  value: unknown
  valueAlign: "left" | "right"
  isLast: boolean
  onChange: (value: unknown) => void
}) {
  const { t } = useI18n()
  const label = getSchemaFieldLabel(name, field)
  const subtitle = getSchemaFieldSubtitle(field)
  const type = resolveFormFieldType(field)

  return (
    <div className={cn("@container", !isLast && "border-b")}>
      <div className="grid gap-3 py-3 @min-[32rem]:grid-cols-[minmax(7rem,10rem)_minmax(0,1fr)] @min-[32rem]:items-center">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <code className="text-sm font-medium">{name}</code>
            {required ? <Badge variant="secondary" className="px-1.5 py-0 text-[10px] uppercase">{t("required")}</Badge> : null}
          </div>
          {subtitle && subtitle !== label ? (
            <p className="mt-0.5 text-sm text-muted-foreground">{subtitle}</p>
          ) : label !== name ? (
            <p className="mt-0.5 text-sm text-muted-foreground">{label}</p>
          ) : null}
        </div>

        <div className={cn("min-w-0", valueAlign === "right" && "text-right")}>
          <ToolArgFieldInput id={name} field={field} type={type} value={value} valueAlign={valueAlign} onChange={onChange} />
        </div>
      </div>
    </div>
  )
}

export function ToolArgFieldInput({
  id,
  field,
  type,
  value,
  valueAlign,
  onChange,
  compact = false,
}: {
  id: string
  field: SchemaField
  type: FormFieldType
  value: unknown
  valueAlign: "left" | "right"
  onChange: (value: unknown) => void
  compact?: boolean
}) {
  const aligned = valueAlign === "right"
  const placeholder = formatDefaultPlaceholder(field)
  const empty = isEmptyFormValue(value)

  if (type === "boolean") {
    return (
      <div className={cn("flex min-h-9 w-full items-center", aligned ? "justify-end" : "justify-start")}>
        <Switch id={id} checked={Boolean(value)} onCheckedChange={onChange} />
      </div>
    )
  }

  if (type === "enum") {
    const options = (field.enum as unknown[]).map(String)
    if (compact) {
      return (
        <Select value={empty ? "__empty__" : String(value)} onValueChange={(next) => onChange(next === "__empty__" ? "" : next)}>
          <SelectTrigger
            id={id}
            className={cn("h-auto min-h-9 w-full py-1", inputMutedClass, aligned && "text-right [&_[data-slot=select-value]]:text-right")}
          >
            <SelectValue placeholder={placeholder} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__empty__">{placeholder}</SelectItem>
            {options.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )
    }
    return (
      <Select value={empty ? "__empty__" : String(value)} onValueChange={(next) => onChange(next === "__empty__" ? "" : next)}>
        <SelectTrigger id={id} className={cn("w-full", inputMutedClass, aligned && "text-right [&_[data-slot=select-value]]:text-right")}>
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__empty__">{placeholder}</SelectItem>
          {options.map((option) => (
            <SelectItem key={option} value={option}>
              {option}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    )
  }

  if (type === "integer" || type === "number") {
    if (compact) {
      return (
        <Textarea
          id={id}
          rows={1}
          inputMode={type === "integer" ? "numeric" : "decimal"}
          value={empty ? "" : String(value)}
          placeholder={placeholder}
          className={cn(compactTextareaClass, inputMutedClass, empty && "text-muted-foreground", aligned && "text-right")}
          onChange={(event) => onChange(event.target.value)}
        />
      )
    }
    return (
      <Input
        id={id}
        type="number"
        step={type === "integer" ? 1 : "any"}
        value={empty ? "" : String(value)}
        placeholder={placeholder}
        className={cn(inputMutedClass, empty && "text-muted-foreground", aligned && "text-right")}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  }

  if (type === "json") {
    const text = empty ? "" : typeof value === "string" ? value : JSON.stringify(value, null, 2)
    return (
      <Textarea
        id={id}
        rows={compact ? 1 : 3}
        value={text}
        placeholder={placeholder}
        className={cn(
          "font-mono text-xs",
          inputMutedClass,
          compact && compactTextareaClass,
          aligned && "text-right",
        )
}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  }

  if (compact) {
    return (
      <Textarea
        id={id}
        rows={1}
        value={empty ? "" : String(value)}
        placeholder={placeholder}
        className={cn(compactTextareaClass, inputMutedClass, empty && "text-muted-foreground", aligned && "text-right")}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  }

  return (
    <Input
      id={id}
      value={empty ? "" : String(value)}
      placeholder={placeholder}
      className={cn(inputMutedClass, empty && "text-muted-foreground", aligned && "text-right")}
      onChange={(event) => onChange(event.target.value)}
    />
  )
}
