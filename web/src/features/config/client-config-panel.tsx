import { useState } from "react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { applyClientConfig, getAggregateLaunch, inspectClientConfig, planClientConfig, undoClientConfig } from "@/lib/api"

const initialEntries = JSON.stringify([
  { name: "mcpstore", kind: "aggregate_http", config: { url: "http://127.0.0.1:18200/mcp" } },
], null, 2)

export function ClientConfigPanel() {
  const [client, setClient] = useState("codex")
  const [path, setPath] = useState("")
  const [entriesText, setEntriesText] = useState(initialEntries)
  const [contentHash, setContentHash] = useState("")
  const [changeId, setChangeId] = useState("")
  const [result, setResult] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)
  const [transport, setTransport] = useState<"stdio" | "streamable-http">("streamable-http")
  const [launch, setLaunch] = useState<unknown>(null)

  function entries() {
    const value = JSON.parse(entriesText)
    if (!Array.isArray(value)) throw new Error("Entries must be a JSON array")
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

  async function loadLaunch() {
    setBusy(true)
    try { setLaunch(await getAggregateLaunch({ transport })) }
    catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

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
          </select>
        </label>
        <label className="grid gap-2">
          <Label>Exact configuration path</Label>
          <Input value={path} onChange={(event) => { setPath(event.target.value); setContentHash("") }} placeholder="/Users/you/.codex/config.toml" />
        </label>
      </div>
      <div className="mt-4 flex flex-wrap items-end gap-2">
        <label className="grid gap-2"><Label>Aggregate transport</Label><select className="h-9 rounded-md border bg-background px-3 text-sm" value={transport} onChange={(event) => setTransport(event.target.value as typeof transport)}><option value="streamable-http">Streamable HTTP</option><option value="stdio">stdio</option></select></label>
        <Button variant="outline" disabled={busy} onClick={() => void loadLaunch()}>Show launch info</Button>
      </div>
      {launch ? <div className="mt-3"><JsonBlock value={launch} /></div> : null}
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
