import {
  getConfigTextPlaceholder,
  useAddServiceForm,
  type AddServiceMode,
  type AddServiceScope,
} from "@/features/services/use-add-service-form"
import { ServiceRestartPolicySelect, ServiceStartupPolicySelect } from "@/features/services/service-lifecycle-fields"
import { DialogFormFooter } from "@/components/shared/dialog-form"
import { Button } from "@/components/ui/button"
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import { getAgentId } from "@/features/agents/model"
import { type AgentItem } from "@/lib/api"
import { cn } from "@/lib/utils"

const MODE_OPTIONS: Array<{ value: AddServiceMode; label: string }> = [
  { value: "stdio", label: "stdio" },
  { value: "streamable-http", label: "streamable-http" },
  { value: "sse", label: "sse" },
  { value: "json", label: "json" },
  { value: "toml", label: "toml" },
]

export function AddServiceForm({
  agents,
  className,
  onAdded,
  onCancel,
  showActions = true,
}: {
  agents: AgentItem[]
  className?: string
  onAdded: () => Promise<void>
  onCancel?: () => void
  showActions?: boolean
}) {
  const { t } = useI18n()
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const {
    agentId,
    mode,
    onSubmit,
    restartPolicy,
    scope,
    setAgentId,
    setMode,
    setRestartPolicy,
    setScope,
    setStartupPolicy,
    startupPolicy,
    submitting,
  } = useAddServiceForm({
    onAdded,
    onBack: () => onCancel?.(),
  })

  const isStdio = mode === "stdio"
  const isTextConfig = mode === "json" || mode === "toml"
  const isHttpLike = mode === "streamable-http" || mode === "sse"

  return (
    <form className={cn("flex min-h-0 flex-1 flex-col", className)} onSubmit={onSubmit}>
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <FieldGroup className="gap-6">
          <div className="grid gap-4 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="name">{t("name")}</FieldLabel>
              <Input id="name" name="name" placeholder="github" required autoFocus />
            </Field>

            <Field>
              <FieldLabel>{t("scope")}</FieldLabel>
              <Tabs
                value={scope}
                onValueChange={(value) => setScope(value as AddServiceScope)}
              >
                <TabsList className="grid w-full grid-cols-2">
                  <TabsTrigger value="store">{t("store")}</TabsTrigger>
                  <TabsTrigger value="agent">{t("agent")}</TabsTrigger>
                </TabsList>
              </Tabs>
              <FieldDescription>{t("fieldHelpScope")}</FieldDescription>
            </Field>
          </div>

          {scope === "agent" ? (
            <Field>
              <FieldLabel htmlFor="agentId">{t("agent")}</FieldLabel>
              {agentIds.length ? (
                <Select
                  value={agentId || "manual"}
                  onValueChange={(value) => setAgentId(value === "manual" ? "" : value)}
                >
                  <SelectTrigger id="agentId" className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="manual">{t("manual")}</SelectItem>
                      {agentIds.map((id) => (
                        <SelectItem key={id} value={id}>
                          {id}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              ) : (
                <Input id="agentId" name="agentId" placeholder="agent-a" required />
              )}
              <FieldDescription>{t("fieldHelpAgent")}</FieldDescription>
            </Field>
          ) : null}

          {scope === "agent" && agentIds.length && !agentId ? (
            <Field>
              <FieldLabel htmlFor="manualAgentId">{t("manualAgentId")}</FieldLabel>
              <Input id="manualAgentId" name="agentId" placeholder="agent-a" required />
            </Field>
          ) : null}

          <FieldSet className="gap-4">
            <FieldLegend variant="label">{t("connection")}</FieldLegend>

            <Tabs
              value={mode}
              onValueChange={(value) => setMode(value as AddServiceMode)}
              orientation="vertical"
              className="flex-col items-stretch gap-4 sm:flex-row sm:items-start"
            >
              <TabsList className="h-fit w-full shrink-0 sm:w-48">
                {MODE_OPTIONS.map((option) => (
                  <TabsTrigger key={option.value} value={option.value} className="justify-start px-3 text-xs sm:text-sm">
                    {option.label}
                  </TabsTrigger>
                ))}
              </TabsList>

              <div className="min-w-0 flex-1">
                <TabsContent value="stdio" className="mt-0 flex flex-col gap-4">
                  <Field>
                    <FieldLabel htmlFor="commandOrUrl-stdio">{t("commandOrUrl")}</FieldLabel>
                    <InputGroup>
                      <InputGroupAddon align="inline-start">stdio</InputGroupAddon>
                      <InputGroupInput
                        id="commandOrUrl-stdio"
                        name="commandOrUrl"
                        placeholder="npx -y @modelcontextprotocol/server-filesystem ."
                        required={isStdio}
                      />
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpCommand")}</FieldDescription>
                  </Field>
                </TabsContent>

                <TabsContent value="streamable-http" className="mt-0">
                  <Field>
                    <FieldLabel htmlFor="commandOrUrl-http">{t("commandOrUrl")}</FieldLabel>
                    <InputGroup>
                      <InputGroupAddon align="inline-start">streamable-http</InputGroupAddon>
                      <InputGroupInput
                        id="commandOrUrl-http"
                        name="commandOrUrl"
                        placeholder="https://example.com/mcp"
                        required={mode === "streamable-http"}
                      />
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpUrl")}</FieldDescription>
                  </Field>
                </TabsContent>

                <TabsContent value="sse" className="mt-0">
                  <Field>
                    <FieldLabel htmlFor="commandOrUrl-sse">{t("commandOrUrl")}</FieldLabel>
                    <InputGroup>
                      <InputGroupAddon align="inline-start">sse</InputGroupAddon>
                      <InputGroupInput
                        id="commandOrUrl-sse"
                        name="commandOrUrl"
                        placeholder="https://example.com/sse"
                        required={mode === "sse"}
                      />
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpUrl")}</FieldDescription>
                  </Field>
                </TabsContent>

                <TabsContent value="json" className="mt-0">
                  <Field>
                    <FieldLabel htmlFor="configText-json">JSON</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea
                        id="configText-json"
                        name="configText"
                        rows={10}
                        className="font-mono text-xs"
                        placeholder={getConfigTextPlaceholder("json")}
                        required={mode === "json"}
                      />
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpJson")}</FieldDescription>
                  </Field>
                </TabsContent>

                <TabsContent value="toml" className="mt-0">
                  <Field>
                    <FieldLabel htmlFor="configText-toml">TOML</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea
                        id="configText-toml"
                        name="configText"
                        rows={10}
                        className="font-mono text-xs"
                        placeholder={getConfigTextPlaceholder("toml")}
                        required={mode === "toml"}
                      />
                    </InputGroup>
                    <FieldDescription>{t("fieldHelpToml")}</FieldDescription>
                  </Field>
                </TabsContent>
              </div>
            </Tabs>
          </FieldSet>

          {!isTextConfig ? (
            <>
              <div className={cn("grid gap-4", isStdio && "sm:grid-cols-2")}>
                <Field>
                  <FieldLabel htmlFor="description">{t("description")}</FieldLabel>
                  <Input id="description" name="description" placeholder={t("optionalDescription")} />
                </Field>
                {isStdio ? (
                  <Field>
                    <FieldLabel htmlFor="workingDir">{t("workingDirectory")}</FieldLabel>
                    <InputGroup>
                      <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
                      <InputGroupInput id="workingDir" name="workingDir" placeholder={t("optional")} />
                    </InputGroup>
                  </Field>
                ) : null}
              </div>

              <FieldSet className="gap-3">
                <FieldLegend variant="label">{t("envVars")}</FieldLegend>
                <Tabs key={isHttpLike ? "http" : "stdio"} defaultValue={isHttpLike ? "headers" : "env"}>
                  <TabsList>
                    <TabsTrigger value="env">{t("env")}</TabsTrigger>
                    <TabsTrigger value="headers">{t("headers")}</TabsTrigger>
                  </TabsList>
                  <TabsContent value="env" className="mt-3">
                    <Field>
                      <FieldLabel htmlFor="env" className="sr-only">
                        {t("envVars")}
                      </FieldLabel>
                      <InputGroup>
                        <InputGroupTextarea id="env" name="env" rows={4} placeholder="TOKEN=..." />
                      </InputGroup>
                    </Field>
                  </TabsContent>
                  <TabsContent value="headers" className="mt-3">
                    <Field>
                      <FieldLabel htmlFor="headers" className="sr-only">
                        {t("headers")}
                      </FieldLabel>
                      <InputGroup>
                        <InputGroupTextarea
                          id="headers"
                          name="headers"
                          rows={4}
                          placeholder="Authorization=Bearer ..."
                        />
                      </InputGroup>
                    </Field>
                  </TabsContent>
                </Tabs>
              </FieldSet>
            </>
          ) : null}

          <FieldSet className="gap-4">
            <FieldLegend variant="label">{t("lifecyclePolicy")}</FieldLegend>
            <FieldDescription>{t("lifecyclePolicyDescription")}</FieldDescription>
            <div className="grid gap-4 sm:grid-cols-2">
              <ServiceStartupPolicySelect value={startupPolicy} onChange={setStartupPolicy} />
              <ServiceRestartPolicySelect value={restartPolicy} onChange={setRestartPolicy} />
            </div>
          </FieldSet>
        </FieldGroup>
      </div>

      {showActions ? (
        <div className="shrink-0 border-t px-6 py-4">
          {onCancel ? (
            <DialogFormFooter
              onCancel={onCancel}
              submitLabel={submitting ? t("adding") : t("navAddService")}
              submitting={submitting}
            />
          ) : (
            <div className="flex justify-end">
              <Button type="submit" disabled={submitting}>
                {submitting ? <Spinner data-icon="inline-start" /> : null}
                {submitting ? t("adding") : t("navAddService")}
              </Button>
            </div>
          )}
        </div>
      ) : null}
    </form>
  )
}
