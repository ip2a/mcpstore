import { SettingsIcon } from "lucide-react"

import { type AppView, useNavItems } from "@/app/app-view"
import { absoluteApiBase, getApiBase } from "@/lib/api/backend"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

function SidebarItem({
  active,
  icon: Icon,
  label,
  onSelect,
}: {
  active: boolean
  icon: typeof SettingsIcon
  label: string
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
        active
          ? "bg-muted font-medium text-foreground"
          : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
      )}
      onClick={onSelect}
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      <span className="truncate">{label}</span>
    </button>
  )
}

export function AppSidebar({
  onOpenSettings,
  onViewChange,
  view,
}: {
  onOpenSettings: () => void
  onViewChange: (view: AppView) => void
  view: AppView
}) {
  const { t } = useI18n()
  const navItems = useNavItems()

  return (
    <aside className="flex w-48 shrink-0 flex-col border-r bg-card/40">
      <div className="flex h-14 shrink-0 items-center border-b px-4">
        <button className="font-mono font-bold" type="button" onClick={() => onViewChange({ name: "home" })}>
          mcpstore
        </button>
      </div>
      <nav className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto p-2">
        {navItems.map((item) => (
          <SidebarItem
            key={item.view.name}
            active={view.name === item.view.name || (view.name === "instance" && item.view.name === "services")}
            icon={item.icon}
            label={item.label}
            onSelect={() => onViewChange(item.view)}
          />
        ))}
      </nav>
      <div className="flex shrink-0 flex-col gap-1 border-t p-2">
        <SidebarItem active={false} icon={SettingsIcon} label={t("settings")} onSelect={onOpenSettings} />
        <div className="flex items-center gap-1.5 px-3 py-1 text-xs text-muted-foreground">
          <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-emerald-500" />
          <span className="truncate font-mono">{absoluteApiBase(getApiBase())}</span>
        </div>
      </div>
    </aside>
  )
}
