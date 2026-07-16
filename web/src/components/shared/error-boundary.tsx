import { Component, type ErrorInfo, type ReactNode } from "react"
import { RefreshCwIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"

interface State {
  error: Error | null
}

/**
 * Catches uncaught errors thrown during render and shows a fallback instead of
 * a blank screen. Intentionally avoids i18n and other context hooks so the
 * fallback can still render when a provider itself is the source of the crash.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary] Uncaught render error:", error, info)
  }

  render() {
    const { error } = this.state
    if (!error) return this.props.children
    return (
      <div className="flex min-h-dvh items-center justify-center p-6">
        <Alert variant="destructive" className="flex w-full max-w-xl flex-col gap-3">
          <AlertTitle>页面渲染出错</AlertTitle>
          <AlertDescription className="break-all">{error.message || error.name || "Unknown error"}</AlertDescription>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/40 p-3 font-mono text-xs">
            {error.stack ?? error.toString()}
          </pre>
          <div>
            <Button variant="outline" onClick={() => window.location.reload()}>
              <RefreshCwIcon data-icon="inline-start" />
              重新加载
            </Button>
          </div>
        </Alert>
      </div>
    )
  }
}
