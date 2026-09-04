import type { ReactNode } from "react"

import { FieldCollapsible } from "@/components/shared/field-collapsible"
import { KeyValuePairsEditor } from "@/components/shared/key-value-pairs-editor"
import { StringListEditor } from "@/components/shared/string-list-editor"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useI18n } from "@/lib/i18n-context"
import {
  HANDSHAKE_MODES,
  getUiTransportMode,
  type HandshakeMode,
  type ServiceConfigFields,
} from "@/features/services/service-config-draft"
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
              <InputGroupAddon align="inline-start">url</InputGroupAddon>
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

          <FieldCollapsible title={t("oauthEnabled")}>
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm text-muted-foreground">{t("oauthEnabledHint")}</span>
                <Switch
                  id="config-oauth-enabled"
                  checked={fields.oauthEnabled}
                  onCheckedChange={(oauthEnabled) => updateFields({ oauthEnabled })}
                  aria-label={t("oauthEnabled")}
                />
              </div>
              {fields.oauthEnabled ? (
                <>
                  <Field>
                    <FieldLabel htmlFor="config-oauth-client-id">{t("oauthClientId")}</FieldLabel>
                    <Input
                      id="config-oauth-client-id"
                      value={fields.oauthClientId}
                      placeholder={t("oauthClientIdHint")}
                      onChange={(event) => updateFields({ oauthClientId: event.target.value })}
                    />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="config-oauth-client-metadata-url">
                      {t("oauthClientMetadataUrl")}
                    </FieldLabel>
                    <Input
                      id="config-oauth-client-metadata-url"
                      type="url"
                      value={fields.oauthClientMetadataUrl}
                      placeholder="https://client.example/mcpstore.json"
                      required={!fields.oauthClientId.trim()}
                      onChange={(event) =>
                        updateFields({ oauthClientMetadataUrl: event.target.value })
                      }
                    />
                    <span className="text-xs text-muted-foreground">
                      {t("oauthClientMetadataUrlHint")}
                    </span>
                  </Field>
                </>
              ) : null}
            </div>
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

      <FieldCollapsible title={t("handshakeMode")}>
        <Field orientation="responsive">
          <FieldLabel htmlFor="config-handshake-mode">{t("handshakeMode")}</FieldLabel>
          <Select
            value={fields.handshakeMode}
            onValueChange={(value) => updateFields({ handshakeMode: value as HandshakeMode })}
          >
            <SelectTrigger id="config-handshake-mode" className="w-44">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {HANDSHAKE_MODES.map((mode) => (
                  <SelectItem key={mode} value={mode}>
                    {t(`handshake${mode.charAt(0).toUpperCase()}${mode.slice(1)}`)}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
        <span className="text-xs text-muted-foreground">{t("handshakeModeHint")}</span>
      </FieldCollapsible>

      {scopeExtra}
    </div>
  )
}
