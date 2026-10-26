import type { LucideIcon } from "lucide-react"
import { SettingsIcon } from "lucide-react"

import { type AppView, useNavItems } from "@/app/app-view"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

function SidebarItem({
  active,
  ariaLabel,
  icon: Icon,
  label,
  onSelect,
}: {
  active: boolean
  ariaLabel?: string
  icon: LucideIcon
  label?: string
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      aria-label={ariaLabel ?? label}
      className={cn(
        "flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
        active
          ? "bg-muted font-medium text-foreground"
          : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
      )}
      onClick={onSelect}
    >
      <Icon aria-hidden="true" className="size-4 shrink-0" />
      {label ? <span className="truncate">{label}</span> : null}
    </button>
  )
}

export function AppSidebar({
  onViewChange,
  view,
}: {
  onViewChange: (view: AppView) => void
  view: AppView
}) {
  const { t } = useI18n()
  const navItems = useNavItems()

  return (
    <aside className="flex w-48 shrink-0 flex-col border-r bg-card/40">
      <nav className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto p-2 pt-3">
        {navItems.map((item) => (
          <SidebarItem
            key={item.view.name}
            active={
              view.name === item.view.name ||
              (view.name === "instance" && item.view.name === "services") ||
              (view.name === "tool" && item.view.name === "tools")
            }
            icon={item.icon}
            label={item.label}
            onSelect={() => onViewChange(item.view)}
          />
        ))}
        <SidebarItem
          active={view.name === "settings"}
          icon={SettingsIcon}
          label={t("settings")}
          onSelect={() => onViewChange({ name: "settings" })}
        />
      </nav>
    </aside>
  )
}
