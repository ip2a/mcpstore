import { useEffect, useState } from "react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { applyClientConfig, getAggregateLaunch, getAggregateStatus, importClientServices, inspectClientConfig, planClientConfig, startAggregate, stopAggregate, undoClientConfig, type AggregateOptions, type AggregateStatus } from "@/lib/api"

const initialEntries = JSON.stringify([
  { name: "mcpstore", kind: "aggregate_http", config: { url: "http://127.0.0.1:1830/mcp" } },
], null, 2)

export function ClientConfigPanel() {
  const [client, setClient] = useState("codex")
  const [path, setPath] = useState("")
  const [entriesText, setEntriesText] = useState(initialEntries)
  const [importNamesText, setImportNamesText] = useState("[]")
  const [contentHash, setContentHash] = useState("")
  const [changeId, setChangeId] = useState("")
  const [result, setResult] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)
  const [transport, setTransport] = useState<"stdio" | "streamable-http">("streamable-http")
  const [scope, setScope] = useState<"store" | "agent">("store")
  const [agentId, setAgentId] = useState("")
  const [host, setHost] = useState("127.0.0.1")
  const [port, setPort] = useState("1830")
  const [pathValue, setPathValue] = useState("/mcp")
  const [launch, setLaunch] = useState<unknown>(null)
  const [aggregateStatus, setAggregateStatus] = useState<AggregateStatus | null>(null)

  function entries() {
    const value = JSON.parse(entriesText)
    if (!Array.isArray(value)) throw new Error("Entries must be a JSON array")
    return value
  }

  function importNames() {
    const value = JSON.parse(importNamesText)
    if (!Array.isArray(value) || value.some((name) => typeof name !== "string")) throw new Error("Import names must be a JSON string array")
    return value
  }

  async function run(action: "inspect" | "plan" | "apply" | "undo") {
    setBusy(true)
    try {
      if (action === "inspect") {
        const value = await inspectClientConfig(client, path)
        setContentHash(value.content_hash)
        setResult(value)
      } else if (action === "plan") {
        const value = await planClientConfig(client, path, entries())
        setContentHash(value.content_hash)
        setResult(value)
      } else if (action === "apply") {
        if (!contentHash) throw new Error("Inspect or preview the current file first")
        if (!window.confirm("Apply this configuration plan? A backup will be created.")) return
        const value = await applyClientConfig(client, path, contentHash, entries())
        setChangeId(value.change_id ?? "")
        setResult(value)
      } else if (action === "undo") {
        const value = await undoClientConfig(changeId)
        setChangeId("")
        setContentHash("")
        setResult(value)
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function importSelected() {
    setBusy(true)
    try {
      if (!window.confirm("Import the selected assistant services into MCPStore? Existing services will not be overwritten.")) return
      setResult(await importClientServices(client, path, importNames()))
    } catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

  function aggregateOptions(): AggregateOptions {
    const parsedPort = Number(port)
    if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) throw new Error("Port must be between 1 and 65535")
    if (scope === "agent" && !agentId.trim()) throw new Error("Agent ID is required for agent scope")
    return { transport, scope, agentId: scope === "agent" ? agentId.trim() : undefined, host, port: parsedPort, path: pathValue }
  }

  async function loadAggregateStatus() {
    try { setAggregateStatus(await getAggregateStatus(aggregateOptions())) }
    catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
  }

  async function loadLaunch() {
    setBusy(true)
    try {
      const options = aggregateOptions()
      setLaunch(await getAggregateLaunch(options))
      setAggregateStatus(await getAggregateStatus(options))
    } catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

  async function toggleAggregate(action: "start" | "stop") {
    setBusy(true)
    try {
      const options = aggregateOptions()
      if (action === "start") await startAggregate(options)
      else await stopAggregate()
      await loadAggregateStatus()
      toast.success(action === "start" ? "Aggregate MCP service started" : "Aggregate MCP service stopped")
    } catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

  useEffect(() => {
    if (scope === "agent" && !agentId.trim()) {
      setAggregateStatus(null)
      return
    }
    void loadAggregateStatus()
    const timer = window.setInterval(() => void loadAggregateStatus(), 5000)
    return () => window.clearInterval(timer)
  }, [transport, scope, agentId, host, port, pathValue])

  return (
    <section className="mt-6 border-t pt-5">
      <SectionHeading title="Programming assistant configuration" titleAs="h2" description="Inspect → preview → confirm → apply → undo" />
      <div className="mt-4 grid gap-4 md:grid-cols-2">
        <label className="grid gap-2">
          <Label>Client</Label>
          <select className="h-9 rounded-md border bg-background px-3 text-sm" value={client} onChange={(event) => { setClient(event.target.value); setContentHash("") }}>
            <option value="codex">Codex</option>
            <option value="claude_code">Claude Code</option>
            <option value="opencode">OpenCode</option>
            <option value="cursor">Cursor</option>
            <option value="claude_desktop">Claude Desktop</option>
          </select>
        </label>
        <label className="grid gap-2">
          <Label>Exact configuration path</Label>
          <Input value={path} onChange={(event) => { setPath(event.target.value); setContentHash("") }} placeholder="/Users/you/.codex/config.toml" />
        </label>
      </div>
      <div className="mt-4 grid gap-4 md:grid-cols-3">
        <label className="grid gap-2"><Label>Aggregate transport</Label><select className="h-9 rounded-md border bg-background px-3 text-sm" value={transport} onChange={(event) => setTransport(event.target.value as typeof transport)}><option value="streamable-http">Streamable HTTP</option><option value="stdio">stdio</option></select></label>
        <label className="grid gap-2"><Label>Scope</Label><select className="h-9 rounded-md border bg-background px-3 text-sm" value={scope} onChange={(event) => setScope(event.target.value as typeof scope)}><option value="store">Store</option><option value="agent">Agent</option></select></label>
        {scope === "agent" ? <label className="grid gap-2"><Label>Agent ID</Label><Input value={agentId} onChange={(event) => setAgentId(event.target.value)} placeholder="agent-id" /></label> : null}
        <label className="grid gap-2"><Label>Host</Label><Input value={host} onChange={(event) => setHost(event.target.value)} /></label>
        <label className="grid gap-2"><Label>HTTP port</Label><Input type="number" min={1} max={65535} value={port} onChange={(event) => setPort(event.target.value)} /></label>
        <label className="grid gap-2"><Label>Path</Label><Input value={pathValue} onChange={(event) => setPathValue(event.target.value)} /></label>
      </div>
      <div className="mt-4 flex flex-wrap items-center gap-2">
        <Button disabled={busy || transport !== "streamable-http" || (scope === "agent" && !agentId.trim())} onClick={() => void toggleAggregate("start")}>Start HTTP aggregate</Button>
        <Button variant="outline" disabled={busy || !aggregateStatus?.running} onClick={() => void toggleAggregate("stop")}>Stop</Button>
        <Button variant="outline" disabled={busy} onClick={() => void loadLaunch()}>Show launch info</Button>
        <Button variant="ghost" disabled={busy} onClick={() => void loadAggregateStatus()}>Refresh status</Button>
        <span className="text-sm text-muted-foreground">Status: {aggregateStatus?.running ? `running${aggregateStatus.pid ? ` (#${aggregateStatus.pid})` : ""}` : "stopped"}</span>
      </div>
      {transport === "stdio" ? <p className="mt-2 text-sm text-muted-foreground">stdio is started by the MCP client; use launch info and client configuration instead of the Web background control.</p> : null}
      {aggregateStatus?.url ? <p className="mt-2 break-all font-mono text-xs text-muted-foreground">{aggregateStatus.url}</p> : null}
      {launch ? <div className="mt-3"><JsonBlock value={launch} /></div> : null}
      <label className="mt-4 grid gap-2">
        <Label>Services to import from assistant (JSON string array)</Label>
        <Textarea className="min-h-16 font-mono text-xs" value={importNamesText} onChange={(event) => setImportNamesText(event.target.value)} placeholder='["my-server"]' />
      </label>
      <div className="mt-2">
        <Button variant="outline" disabled={busy || !path} onClick={() => void importSelected()}>Import selected services</Button>
      </div>
      <label className="mt-4 grid gap-2">
        <Label>Entries</Label>
        <Textarea className="min-h-40 font-mono text-xs" value={entriesText} onChange={(event) => { setEntriesText(event.target.value); setContentHash("") }} />
      </label>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button variant="outline" disabled={busy || !path} onClick={() => void run("inspect")}>Inspect</Button>
        <Button variant="outline" disabled={busy || !path} onClick={() => void run("plan")}>Preview</Button>
        <Button disabled={busy || !path || !contentHash} onClick={() => void run("apply")}>Apply</Button>
        <Button variant="destructive" disabled={busy || !changeId} onClick={() => void run("undo")}>Undo</Button>
      </div>
      {result ? <div className="mt-4"><JsonBlock value={result} /></div> : null}
    </section>
  )
}
