import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { QueryClientProvider } from "@tanstack/react-query"
import { BrowserRouter } from "react-router-dom"
import { ThemeProvider } from "next-themes"
import "./index.css"
import { queryClient } from "@/app/query-client"
import { ErrorBoundary } from "@/components/shared/error-boundary"
import { PanelTokenGate } from "@/components/shared/panel-token-gate"
import { I18nProvider } from "@/lib/i18n-provider"
import { App } from "./App"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
      <QueryClientProvider client={queryClient}>
        {/* Above I18nProvider: its meta query is what 401s on an
            authenticated panel, and the gate must render in that state. */}
        <PanelTokenGate />
        <I18nProvider>
          <BrowserRouter>
            <ErrorBoundary>
              <App />
            </ErrorBoundary>
          </BrowserRouter>
        </I18nProvider>
      </QueryClientProvider>
    </ThemeProvider>
  </StrictMode>,
)
