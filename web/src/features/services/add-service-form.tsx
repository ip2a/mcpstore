import { getAgentId } from "@/features/agents/model"
import { useAddServiceForm, type AddServiceScope, type AddServiceTransport } from "@/features/services/use-add-service-form"
import { ServiceRestartPolicySelect, ServiceStartupPolicySelect } from "@/features/services/service-lifecycle-fields"
import { OptionToggleGroup } from "@/components/shared/option-toggle-group"
import { Button } from "@/components/ui/button"
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldTitle } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem } from "@/lib/api"

const TRANSPORT_OPTIONS: Array<{ value: AddServiceTransport; label: string }> = [
  { value: "stdio", label: "stdio" },
  { value: "streamable-http", label: "streamable-http" },
  { value: "sse", label: "sse" },
]

export function AddServiceForm({
  agents,
  onAdded,
  onCancel,
  showActions = true,
}: {
  agents: AgentItem[]
  onAdded: () => Promise<void>
  onCancel?: () => void
  showActions?: boolean
}) {
  const { t } = useI18n()
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const scopeOptions: Array<{ value: AddServiceScope; label: string }> = [
    { value: "store", label: t("store") },
    { value: "agent", label: t("agent") },
  ]
  const {
    agentId,
    onSubmit,
    restartPolicy,
    scope,
    setAgentId,
    setRestartPolicy,
    setScope,
    setStartupPolicy,
    setTransport,
    startupPolicy,
    submitting,
    transport,
  } = useAddServiceForm({
    onAdded,
    onBack: () => onCancel?.(),
  })

  return (
    <form onSubmit={onSubmit}>
      <FieldGroup className="gap-5">
        <section className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor="name">{t("name")}</FieldLabel>
            <Input id="name" name="name" placeholder="github" required />
          </Field>

          <div className="flex flex-wrap items-center gap-x-5 gap-y-3">
            <div className="flex items-center gap-2">
              <FieldTitle className="text-muted-foreground">{t("scope")}</FieldTitle>
              <OptionToggleGroup value={scope} options={scopeOptions} onChange={setScope} />
            </div>

            {scope === "agent" ? (
              <div className="flex min-w-[12rem] flex-1 items-center gap-2">
                <FieldTitle className="shrink-0 text-muted-foreground">{t("agent")}</FieldTitle>
                {agentIds.length ? (
                  <Select
                    value={agentId || "manual"}
                    onValueChange={(value) => setAgentId(value === "manual" ? "" : value)}
                  >
                    <SelectTrigger className="h-8 min-w-[10rem]">
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
                  <Input className="h-8" name="agentId" placeholder="agent-a" required />
                )}
              </div>
            ) : null}
          </div>

          {scope === "agent" && agentIds.length && !agentId ? (
            <Field>
              <FieldLabel htmlFor="agentId">{t("manualAgentId")}</FieldLabel>
              <Input id="agentId" name="agentId" className="max-w-md" placeholder="agent-a" required />
            </Field>
          ) : null}
        </section>

        <section className="flex flex-col gap-4 rounded-lg border p-4">
          <div className="flex flex-wrap items-center gap-2">
            <FieldTitle className="text-muted-foreground">{t("transport")}</FieldTitle>
            <OptionToggleGroup value={transport} options={TRANSPORT_OPTIONS} onChange={setTransport} />
          </div>

          <Field>
            <FieldLabel htmlFor="commandOrUrl">{t("commandOrUrl")}</FieldLabel>
            <InputGroup>
              <InputGroupAddon align="inline-start">{transport}</InputGroupAddon>
              <InputGroupInput
                id="commandOrUrl"
                name="commandOrUrl"
                placeholder={transport === "stdio" ? "npx -y @modelcontextprotocol/server-filesystem ." : "https://example.com/mcp"}
                required
              />
            </InputGroup>
          </Field>
        </section>

        <section className="grid gap-4 md:grid-cols-2">
          <Field>
            <FieldLabel htmlFor="description">{t("description")}</FieldLabel>
            <Input id="description" name="description" placeholder={t("optionalDescription")} />
          </Field>
          <Field>
            <FieldLabel htmlFor="workingDir">{t("workingDirectory")}</FieldLabel>
            <InputGroup>
              <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
              <InputGroupInput id="workingDir" name="workingDir" placeholder={t("optional")} />
            </InputGroup>
          </Field>
        </section>

        <Tabs defaultValue="env">
          <TabsList>
            <TabsTrigger value="env">{t("env")}</TabsTrigger>
            <TabsTrigger value="headers">{t("headers")}</TabsTrigger>
          </TabsList>
          <TabsContent value="env">
            <Field>
              <FieldLabel htmlFor="env">{t("envVars")}</FieldLabel>
              <InputGroup>
                <InputGroupTextarea id="env" name="env" placeholder="TOKEN=..." />
              </InputGroup>
            </Field>
          </TabsContent>
          <TabsContent value="headers">
            <Field>
              <FieldLabel htmlFor="headers">{t("headers")}</FieldLabel>
              <InputGroup>
                <InputGroupTextarea id="headers" name="headers" placeholder="Authorization=Bearer ..." />
              </InputGroup>
            </Field>
          </TabsContent>
        </Tabs>

        <section className="rounded-lg border p-4">
          <FieldTitle className="mb-1">{t("lifecyclePolicy")}</FieldTitle>
          <FieldDescription className="mb-4">
            {t("lifecyclePolicyDescription")}
          </FieldDescription>
          <div className="grid gap-4 md:grid-cols-2">
            <ServiceStartupPolicySelect value={startupPolicy} onChange={setStartupPolicy} />
            <ServiceRestartPolicySelect value={restartPolicy} onChange={setRestartPolicy} />
          </div>
        </section>

        {showActions ? (
          <div className="flex justify-end gap-2 border-t pt-4">
            {onCancel ? (
              <Button type="button" variant="outline" onClick={onCancel}>
                {t("cancel")}
              </Button>
            ) : null}
            <Button type="submit" disabled={submitting}>
              {submitting ? <Spinner data-icon="inline-start" /> : null}
              {submitting ? t("adding") : t("navAddService")}
            </Button>
          </div>
        ) : null}
      </FieldGroup>
    </form>
  )
}
