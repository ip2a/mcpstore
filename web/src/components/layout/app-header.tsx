import { type AppView } from "@/app/app-view"
import { AppHeaderNav } from "@/components/layout/app-header-nav"
import { type AgentItem } from "@/lib/api"

export function AppHeader({
  agents,
  onAdded,
  onBack,
  onOpenSettings,
  onViewChange,
  pageTitle,
  view,
}: {
  agents: AgentItem[]
  onAdded: () => Promise<void>
  onBack: () => void
  onOpenSettings: () => void
  onViewChange: (view: AppView) => void
  pageTitle: string
  view: AppView
}) {
  return (
    <header className="flex min-h-16 items-center justify-between gap-4 border-b py-3">
      <div className="flex min-w-0 shrink items-center gap-3">
        <button className="font-mono font-bold" type="button" onClick={() => onViewChange({ name: "services" })}>
          mcpstore
        </button>
        <div className="h-5 w-px bg-border" />
        <span className="truncate text-sm font-semibold">{pageTitle}</span>
      </div>

      <AppHeaderNav
        agents={agents}
        view={view}
        onAdded={onAdded}
        onBack={onBack}
        onOpenSettings={onOpenSettings}
        onViewChange={onViewChange}
      />
    </header>
  )
}
