import { useEffect, useState, type ReactNode } from "react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { getMcpHubDescriptor, getMcpHubStatus, startMcpHub, stopMcpHub, type McpHubOptions, type McpHubStatus, type ScopeRef } from "@/lib/api"

function ConfigModule({
  children,
  description,
  title,
}: {
  children: ReactNode
  description?: string
  title: string
}) {
  return (
    <div className="flex flex-col gap-4 rounded-lg border border-border/60 bg-muted/10 p-4">
      <SectionHeading title={title} titleAs="h2" description={description} />
      {children}
    </div>
  )
}

export function ClientConfigPanel({ scope: scopeProp, open, onOpenChange }: { scope?: ScopeRef; open?: boolean; onOpenChange?: (open: boolean) => void }) {
  const [busy, setBusy] = useState(false)
  const [transport, setTransport] = useState<"stdio" | "streamable-http">("streamable-http")
  const [scopeState, setScopeState] = useState<"store" | "agent">("store")
  const [agentIdState, setAgentIdState] = useState("")
  // 传入 scope 时锁定到该作用域（per-scope 模式，隐藏选择器）；不传则用内部选择器（全局模式）。
  const locked = Boolean(scopeProp)
  const scope: "store" | "agent" = scopeProp
    ? scopeProp.type === "agent"
      ? "agent"
      : "store"
    : scopeState
  const agentId = scopeProp?.type === "agent" ? scopeProp.agent_id : agentIdState
  const [host, setHost] = useState("127.0.0.1")
  const [port, setPort] = useState("1830")
  const [pathValue, setPathValue] = useState("/mcp")
  const [launch, setLaunch] = useState<unknown>(null)
  const [mcpHubOpen, setMcpHubOpen] = useState(false)
  const [mcpHubStatus, setMcpHubStatus] = useState<McpHubStatus | null>(null)

  function mcpHubOptions(): McpHubOptions {
    const parsedPort = Number(port)
    if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) throw new Error("Port must be between 1 and 65535")
    if (scope === "agent" && !agentId.trim()) throw new Error("Agent ID is required for agent scope")
    return { transport, scope, agentId: scope === "agent" ? agentId.trim() : undefined, host, port: parsedPort, path: pathValue }
  }

  async function loadMcpHubStatus() {
    try { setMcpHubStatus(await getMcpHubStatus(mcpHubOptions())) }
    catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
  }

  async function loadLaunch() {
    setBusy(true)
    try {
      const options = mcpHubOptions()
      setLaunch(await getMcpHubDescriptor(options))
      setMcpHubStatus(await getMcpHubStatus(options))
    } catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

  async function toggleMcpHub(action: "start" | "stop") {
    setBusy(true)
    try {
      const options = mcpHubOptions()
      if (action === "start") await startMcpHub(options)
      else await stopMcpHub()
      await loadMcpHubStatus()
      toast.success(action === "start" ? "MCP Hub Service started" : "MCP Hub Service stopped")
    } catch (error) { toast.error(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }

  useEffect(() => {
    if (scope === "agent" && !agentId.trim()) {
      setMcpHubStatus(null)
      return
    }
    void loadMcpHubStatus()
    const timer = window.setInterval(() => void loadMcpHubStatus(), 5000)
    return () => window.clearInterval(timer)
  }, [transport, scope, agentId, host, port, pathValue])

  return (
    <section className={open !== undefined ? "hidden" : "mt-6 flex flex-col gap-4 border-t pt-5"}>
      <SectionHeading title="Programming assistant configuration" titleAs="h2" />

      <div className="flex min-w-0 items-baseline justify-between gap-4 border-b py-3">
        <span className="shrink-0 text-sm text-muted-foreground">作用域聚合mcp</span>
        <Dialog open={open ?? mcpHubOpen} onOpenChange={onOpenChange ?? setMcpHubOpen}>
          <DialogTrigger asChild>
            <Button variant="outline" className="h-8 min-w-28 justify-between">
              <span>{mcpHubStatus?.running ? "running" : "stopped"}</span>
              <span className="text-xs text-muted-foreground">配置</span>
            </Button>
          </DialogTrigger>
          <DialogContent className="max-h-[min(720px,calc(100vh-2rem))] overflow-y-auto sm:max-w-2xl">
            <DialogHeader>
              <DialogTitle>作用域聚合mcp</DialogTitle>
              <DialogDescription>Run or inspect the bundled HTTP endpoint for this scope.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 md:grid-cols-2">
              <label className="grid gap-2"><Label>MCP Hub transport</Label><select className="h-9 rounded-md border bg-background px-3 text-sm" value={transport} onChange={(event) => setTransport(event.target.value as typeof transport)}><option value="streamable-http">Streamable HTTP</option><option value="stdio">stdio</option></select></label>
              {!locked ? <label className="grid gap-2"><Label>Scope</Label><select className="h-9 rounded-md border bg-background px-3 text-sm" value={scope} onChange={(event) => setScopeState(event.target.value as typeof scopeState)}><option value="store">Store</option><option value="agent">Agent</option></select></label> : null}
              {!locked && scope === "agent" ? <label className="grid gap-2"><Label>Agent ID</Label><Input value={agentId} onChange={(event) => setAgentIdState(event.target.value)} placeholder="agent-id" /></label> : null}
              <label className="grid gap-2"><Label>Host</Label><Input value={host} onChange={(event) => setHost(event.target.value)} /></label>
              <label className="grid gap-2"><Label>HTTP port</Label><Input type="number" min={1} max={65535} value={port} onChange={(event) => setPort(event.target.value)} /></label>
              <label className="grid gap-2"><Label>Path</Label><Input value={pathValue} onChange={(event) => setPathValue(event.target.value)} /></label>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Button disabled={busy || transport !== "streamable-http" || (scope === "agent" && !agentId.trim())} onClick={() => void toggleMcpHub("start")}>Start MCP Hub</Button>
              <Button variant="outline" disabled={busy || !mcpHubStatus?.running} onClick={() => void toggleMcpHub("stop")}>Stop</Button>
              <Button variant="outline" disabled={busy} onClick={() => void loadLaunch()}>Show launch info</Button>
              <Button variant="ghost" disabled={busy} onClick={() => void loadMcpHubStatus()}>Refresh status</Button>
              <span className="text-sm text-muted-foreground">Status: {mcpHubStatus?.running ? `running${mcpHubStatus.pid ? ` (#${mcpHubStatus.pid})` : ""}` : "stopped"}</span>
            </div>
            {transport === "stdio" ? <p className="text-sm text-muted-foreground">stdio is started by the MCP client; use launch info and client configuration instead of the Web background control.</p> : null}
            {mcpHubStatus?.url ? <p className="break-all font-mono text-xs text-muted-foreground">{mcpHubStatus.url}</p> : null}
            {launch ? <JsonBlock value={launch} /> : null}
            <DialogFooter showCloseButton />
          </DialogContent>
        </Dialog>
      </div>

    </section>
  )
}
