import { ArrowLeftIcon, PlusIcon, SettingsIcon } from "lucide-react"
import { Button } from "@/components/ui/button"

export type AppView =
  | { name: "services" }
  | { name: "agents" }
  | { name: "tools" }
  | { name: "config" }
  | { name: "cache" }
  | { name: "add" }
  | { name: "service"; serviceName: string }

const navItems: Array<{ view: AppView; label: string }> = [
  { view: { name: "services" }, label: "服务" },
  { view: { name: "agents" }, label: "Agent" },
  { view: { name: "tools" }, label: "工具" },
  { view: { name: "config" }, label: "配置" },
  { view: { name: "cache" }, label: "缓存" },
]

export function viewTitle(view: AppView): string {
  if (view.name === "service") return view.serviceName
  if (view.name === "add") return "添加服务"
  return navItems.find((item) => item.view.name === view.name)?.label || "服务"
}

export function AppHeader({ onOpenSettings, onViewChange, pageTitle, view }: { onOpenSettings: () => void; onViewChange: (view: AppView) => void; pageTitle: string; view: AppView }) {
  const isHome = view.name === "services"

  return (
    <header className="mb-3 flex min-h-16 items-center justify-between gap-4 border-b py-3">
      <div className="flex min-w-0 items-center gap-3">
        <button className="font-mono font-bold" type="button" onClick={() => onViewChange({ name: "services" })}>
          mcpstore
        </button>
        <div className="h-5 w-px bg-border" />
        <span className="truncate text-sm font-semibold">{pageTitle}</span>
      </div>

      <nav className="flex flex-wrap items-center justify-end gap-2">
        {!isHome ? (
          <Button type="button" variant="outline" size="sm" onClick={() => onViewChange({ name: "services" })}>
            <ArrowLeftIcon data-icon="inline-start" />
            返回
          </Button>
        ) : null}
        {navItems.map((item) =>
          view.name === item.view.name ? null : (
            <Button key={item.view.name} variant="outline" size="sm" onClick={() => onViewChange(item.view)}>
              {item.label}
            </Button>
          ),
        )}
        <Button variant="outline" size="sm" onClick={() => onViewChange({ name: "add" })}>
          <PlusIcon data-icon="inline-start" />
          添加
        </Button>
        <Button variant="outline" size="sm" onClick={onOpenSettings}>
          <SettingsIcon data-icon="inline-start" />
          设置
        </Button>
      </nav>
    </header>
  )
}
