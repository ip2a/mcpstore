import type { ReactNode } from "react"

import { FieldCollapsible } from "@/components/shared/field-collapsible"
import { KeyValuePairsEditor } from "@/components/shared/key-value-pairs-editor"
import { StringListEditor } from "@/components/shared/string-list-editor"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { useI18n } from "@/lib/i18n-context"
import { getUiTransportMode, type ServiceConfigFields } from "@/features/services/service-config-draft"
import { cn } from "@/lib/utils"

export function ServiceConfigFormFields({
  className,
  scopeExtra,
  fields,
  onFieldsChange,
}: {
  className?: string
  scopeExtra?: ReactNode
  fields: ServiceConfigFields
  onFieldsChange: (fields: ServiceConfigFields) => void
}) {
  const { t } = useI18n()
  const connectionMode = getUiTransportMode(fields.transport)
  const isStdio = connectionMode === "stdio"

  function updateFields(partial: Partial<ServiceConfigFields>) {
    onFieldsChange({ ...fields, ...partial })
  }

  return (
    <div className={cn("flex flex-col gap-4", className)}>
      {isStdio ? (
        <>
          <Field>
            <FieldLabel htmlFor="config-command">{t("command")}</FieldLabel>
            <Input
              id="config-command"
              value={fields.command}
              placeholder="npx"
              required
              onChange={(event) => updateFields({ command: event.target.value })}
            />
          </Field>

          <Field>
            <FieldLabel>{t("args")}</FieldLabel>
            <StringListEditor
              idPrefix="config-args"
              defaultEmptyRow
              value={fields.argsText}
              placeholder="-y"
              onChange={(argsText) => updateFields({ argsText })}
            />
          </Field>

          <FieldCollapsible title={t("envVars")}>
            <KeyValuePairsEditor
              idPrefix="config-env"
              defaultEmptyRow
              value={fields.envText}
              keyPlaceholder="TOKEN"
              valuePlaceholder="..."
              onChange={(envText) => updateFields({ envText })}
            />
          </FieldCollapsible>

          <FieldCollapsible title={t("workingDirectory")}>
            <InputGroup>
              <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
              <InputGroupInput
                id="config-workingDir"
                value={fields.workingDir}
                placeholder={t("optional")}
                onChange={(event) => updateFields({ workingDir: event.target.value })}
              />
            </InputGroup>
          </FieldCollapsible>
        </>
      ) : (
        <>
          <Field>
            <FieldLabel htmlFor="config-url">{t("httpEndpoint")}</FieldLabel>
            <InputGroup>
              <InputGroupAddon align="inline-start">http</InputGroupAddon>
              <InputGroupInput
                id="config-url"
                value={fields.url}
                placeholder="https://example.com/mcp"
                required
                onChange={(event) => updateFields({ url: event.target.value })}
              />
            </InputGroup>
          </Field>

          <FieldCollapsible title={t("headers")}>
            <KeyValuePairsEditor
              idPrefix="config-headers"
              defaultEmptyRow
              valueField="textarea"
              value={fields.headersText}
              keyPlaceholder="Authorization"
              valuePlaceholder="Bearer ..."
              onChange={(headersText) => updateFields({ headersText })}
            />
          </FieldCollapsible>
        </>
      )}

      <FieldCollapsible title={t("description")}>
        <Input
          id="config-description"
          value={fields.description}
          placeholder={t("optionalDescription")}
          onChange={(event) => updateFields({ description: event.target.value })}
        />
      </FieldCollapsible>

      {scopeExtra}
    </div>
  )
}
