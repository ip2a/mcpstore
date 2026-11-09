import type { LucideIcon } from "lucide-react"

import { type AppView, useNavSections } from "@/app/app-view"
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

function SidebarGroupLabel({ children, className }: { children: string; className?: string }) {
  return (
    <div
      className={cn("px-3 pb-0.5 pt-3 text-[11px] font-medium text-muted-foreground/70", className)}
    >
      {children}
    </div>
  )
}

function isItemActive(view: AppView, viewName: AppView["name"]): boolean {
  return (
    view.name === viewName ||
    (view.name === "instance" && viewName === "services") ||
    (view.name === "tool" && viewName === "tools") ||
    (view.name === "resource" && viewName === "resources") ||
    (view.name === "prompt" && viewName === "prompts")
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
  const sections = useNavSections()

  return (
    <aside className="flex w-48 shrink-0 flex-col border-r bg-card/40">
      <nav className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2 pt-3 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
        {sections.map((section, index) => (
          <div key={section.labelKey} className="flex flex-col gap-0.5 pb-1">
            <SidebarGroupLabel className={index === 0 ? "pt-0" : undefined}>
              {t(section.labelKey)}
            </SidebarGroupLabel>
            {section.items.map((item) => (
              <SidebarItem
                key={item.view.name}
                active={isItemActive(view, item.view.name)}
                icon={item.icon}
                label={item.label}
                onSelect={() => onViewChange(item.view)}
              />
            ))}
          </div>
        ))}
      </nav>
    </aside>
  )
}
