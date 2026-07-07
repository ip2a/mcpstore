import { DetailHeader } from "@/components/shared/detail-header"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { getAgentId } from "@/features/agents/model"
import { useAddServiceForm, type AddServiceScope, type AddServiceTransport } from "@/features/services/use-add-service-form"
import { type AgentItem } from "@/lib/api"

export function AddServiceView({ agents, onBack, onAdded }: { agents: AgentItem[]; onBack: () => void; onAdded: () => Promise<void> }) {
  const agentIds = agents.map(getAgentId).filter(Boolean)
  const { agentId, onSubmit, scope, setAgentId, setScope, setTransport, submitting, transport } = useAddServiceForm({ onAdded, onBack })

  return (
    <>
      <DetailHeader eyebrow="添加服务" title="New MCP Service" actions={<Button variant="outline" onClick={onBack}>Back</Button>} />
      <PanelCard>
        <SectionHeading title="Service Config" titleAs="h2" className="border-b-0 pb-0" />
        <form onSubmit={onSubmit}>
          <FieldGroup>
              <div className="grid gap-4 md:grid-cols-2">
                <Field>
                  <FieldLabel htmlFor="name">Name</FieldLabel>
                  <Input id="name" name="name" placeholder="github" required />
                </Field>
                <Field>
                  <FieldLabel>Scope</FieldLabel>
                  <Select value={scope} onValueChange={(value) => setScope(value as AddServiceScope)}>
                    <SelectTrigger><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="store">Store</SelectItem>
                        <SelectItem value="agent">Agent</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <Field data-disabled={scope === "store"}>
                  <FieldLabel>Agent ID</FieldLabel>
                  {agentIds.length ? (
                    <Select value={agentId || "manual"} onValueChange={(value) => setAgentId(value === "manual" ? "" : value)} disabled={scope === "store"}>
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          <SelectItem value="manual">Manual</SelectItem>
                          {agentIds.map((id) => <SelectItem key={id} value={id}>{id}</SelectItem>)}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input name="agentId" placeholder="agent-a" disabled={scope === "store"} required={scope === "agent"} />
                  )}
                </Field>
                <Field>
                  <FieldLabel>Transport</FieldLabel>
                  <Select value={transport} onValueChange={(value) => setTransport(value as AddServiceTransport)}>
                    <SelectTrigger><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="stdio">stdio</SelectItem>
                        <SelectItem value="streamable-http">streamable-http</SelectItem>
                        <SelectItem value="sse">sse</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
              </div>

              {scope === "agent" && agentIds.length ? (
                <Field>
                  <FieldLabel htmlFor="agentId">Manual Agent ID</FieldLabel>
                  <Input id="agentId" name="agentId" placeholder="agent-a" disabled={Boolean(agentId)} required={!agentId} />
                </Field>
              ) : null}

              <Field>
                <FieldLabel htmlFor="commandOrUrl">Command or URL</FieldLabel>
                <InputGroup>
                  <InputGroupAddon align="inline-start">{transport}</InputGroupAddon>
                  <InputGroupInput id="commandOrUrl" name="commandOrUrl" placeholder={transport === "stdio" ? "npx -y @modelcontextprotocol/server-filesystem ." : "https://example.com/mcp"} required />
                </InputGroup>
              </Field>

              <Field>
                <FieldLabel htmlFor="description">Description</FieldLabel>
                <Input id="description" name="description" placeholder="Optional description" />
              </Field>

              <Field>
                <FieldLabel htmlFor="workingDir">Working directory</FieldLabel>
                <InputGroup>
                  <InputGroupAddon align="inline-start">cwd</InputGroupAddon>
                  <InputGroupInput id="workingDir" name="workingDir" placeholder="Optional" />
                </InputGroup>
              </Field>

              <Tabs defaultValue="env">
                <TabsList>
                  <TabsTrigger value="env">Env</TabsTrigger>
                  <TabsTrigger value="headers">Headers</TabsTrigger>
                </TabsList>
                <TabsContent value="env">
                  <Field>
                    <FieldLabel htmlFor="env">Env vars</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea id="env" name="env" placeholder="TOKEN=..." />
                    </InputGroup>
                  </Field>
                </TabsContent>
                <TabsContent value="headers">
                  <Field>
                    <FieldLabel htmlFor="headers">Headers</FieldLabel>
                    <InputGroup>
                      <InputGroupTextarea id="headers" name="headers" placeholder="Authorization=Bearer ..." />
                    </InputGroup>
                  </Field>
                </TabsContent>
              </Tabs>

              <div className="flex justify-end gap-2">
                <Button type="button" variant="outline" onClick={onBack}>Cancel</Button>
                <Button type="submit" disabled={submitting}>
                  {submitting ? <Spinner data-icon="inline-start" /> : null}
                  {submitting ? "Adding" : "Add Service"}
                </Button>
              </div>
          </FieldGroup>
        </form>
      </PanelCard>
    </>
  )
}
