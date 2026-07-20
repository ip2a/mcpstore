import { useEffect, useState } from "react"

import { KeyValuePairsEditor } from "@/components/shared/key-value-pairs-editor"
import { useEditServiceForm } from "@/features/services/use-edit-service-form"
import type { AddServiceTransport } from "@/features/services/use-add-service-form"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance } from "@/lib/api"

export function EditServiceForm({
  onCancel,
  onUpdated,
  service,
}: {
  onCancel: () => void
  onUpdated: () => Promise<void>
  service: ServiceInstance
}) {
  const { t } = useI18n()
  const { defaults, onSubmit, setTransport, submitting, transport } = useEditServiceForm({
    onCancel,
    onUpdated,
    service,
  })
  const [envText, setEnvText] = useState(defaults.env)
  const [headersText, setHeadersText] = useState(defaults.headers)

  useEffect(() => {
    setEnvText(defaults.env)
    setHeadersText(defaults.headers)
  }, [defaults.env, defaults.headers])

  return (
    <form onSubmit={onSubmit}>
      <input type="hidden" name="env" value={envText} />
      <input type="hidden" name="headers" value={headersText} />
      <FieldGroup>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field>
            <FieldLabel>{t("name")}</FieldLabel>
            <Input value={service.service_name} readOnly />
          </Field>
          <Field>
            <FieldLabel>{t("agentScope")}</FieldLabel>
            <Input
              value={service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`}
              readOnly
            />
          </Field>
        </div>
        <Field>
          <FieldLabel>{t("transport")}</FieldLabel>
          <Select value={transport} onValueChange={(value) => setTransport(value as AddServiceTransport)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="stdio">stdio</SelectItem>
                <SelectItem value="streamable-http">streamable-http</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>

        <Field>
          <FieldLabel htmlFor="edit-commandOrUrl">{t("commandOrUrl")}</FieldLabel>
          <InputGroup>
            <InputGroupAddon align="inline-start">{transport}</InputGroupAddon>
            <InputGroupInput
              id="edit-commandOrUrl"
              name="commandOrUrl"
              defaultValue={defaults.commandOrUrl}
              placeholder={transport === "stdio" ? "npx -y @modelcontextprotocol/server-filesystem ." : "https://example.com/mcp"}
              required
            />
          </InputGroup>
        </Field>

        <Field>
          <FieldLabel htmlFor="edit-description">{t("description")}</FieldLabel>
          <Input id="edit-description" name="description" defaultValue={defaults.description} placeholder={t("optionalDescription")} />
        </Field>

        <Field>
          <FieldLabel htmlFor="edit-workingDir">{t("workingDirectory")}</FieldLabel>
          <InputGroup>
            <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
            <InputGroupInput id="edit-workingDir" name="workingDir" defaultValue={defaults.workingDir} placeholder={t("optional")} />
          </InputGroup>
        </Field>

        <Tabs defaultValue="env">
          <TabsList>
            <TabsTrigger value="env">{t("env")}</TabsTrigger>
            <TabsTrigger value="headers">{t("headers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="env">
            <Field>
              <FieldLabel>{t("envVars")}</FieldLabel>
              <KeyValuePairsEditor
                idPrefix="edit-env"
                defaultEmptyRow
                value={envText}
                keyPlaceholder="TOKEN"
                valuePlaceholder="..."
                onChange={setEnvText}
              />
            </Field>
          </TabsContent>
          <TabsContent value="headers">
            <Field>
              <FieldLabel>{t("headers")}</FieldLabel>
              <KeyValuePairsEditor
                idPrefix="edit-headers"
                defaultEmptyRow
                valueField="textarea"
                value={headersText}
                keyPlaceholder="Authorization"
                valuePlaceholder="Bearer ..."
                onChange={setHeadersText}
              />
            </Field>
          </TabsContent>
        </Tabs>

        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onCancel}>
            {t("cancel")}
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? <Spinner data-icon="inline-start" /> : null}
            {submitting ? t("saving") : t("save")}
          </Button>
        </div>
      </FieldGroup>
    </form>
  )
}
