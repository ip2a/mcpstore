import type { LucideIcon } from "lucide-react"

import { type AppView, type NavItem, useNavItems } from "@/app/app-view"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type NavGroupKey = "work" | "extensions" | "primitives" | "observability" | "config"

const VIEW_GROUP: Partial<Record<string, NavGroupKey>> = {
  home: "work",
  services: "work",
  clients: "work",
  gateway: "work",
  plugins: "extensions",
  tools: "primitives",
  resources: "primitives",
  prompts: "primitives",
  history: "observability",
  traces: "observability",
  cache: "observability",
  settings: "config",
}

const GROUP_LABEL_KEY: Record<NavGroupKey, I18nKey> = {
  work: "navGroupWork",
  extensions: "navGroupExtensions",
  primitives: "navGroupPrimitives",
  observability: "navGroupObservability",
  config: "navGroupConfig",
}

type NavSection =
  | { type: "items"; items: NavItem[] }
  | { type: "group"; key: NavGroupKey; items: NavItem[] }

function buildSections(navItems: NavItem[]): NavSection[] {
  const sections: NavSection[] = []
  for (const item of navItems) {
    const group = VIEW_GROUP[item.view.name]
    const last = sections[sections.length - 1]
    if (group) {
      if (last?.type === "group" && last.key === group) {
        last.items.push(item)
      } else {
        sections.push({ type: "group", key: group, items: [item] })
      }
    } else if (last?.type === "items") {
      last.items.push(item)
    } else {
      sections.push({ type: "items", items: [item] })
    }
  }
  return sections
}

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

function SidebarGroupLabel({
  children,
  className,
}: {
  children: string
  className?: string
}) {
  return (
    <div
      className={cn(
        "px-3 pb-0.5 pt-3 text-[11px] font-medium text-muted-foreground/70",
        className,
      )}
    >
      {children}
    </div>
  )
}

function isItemActive(view: AppView, item: NavItem): boolean {
  return (
    view.name === item.view.name ||
    (view.name === "instance" && item.view.name === "services") ||
    (view.name === "tool" && item.view.name === "tools") ||
    (view.name === "resource" && item.view.name === "resources") ||
    (view.name === "prompt" && item.view.name === "prompts")
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
  const sections = buildSections(useNavItems())

  function renderItems(items: NavItem[]) {
    return items.map((item) => (
      <SidebarItem
        key={item.view.name}
        active={isItemActive(view, item)}
        icon={item.icon}
        label={item.label}
        onSelect={() => onViewChange(item.view)}
      />
    ))
  }

  return (
    <aside className="flex w-48 shrink-0 flex-col border-r bg-card/40">
      <nav className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2 pt-3">
        {sections.map((section, index) =>
          section.type === "group" ? (
            <div key={section.key} className="flex flex-col gap-0.5 pb-1">
              <SidebarGroupLabel className={index === 0 ? "pt-0" : undefined}>
                {t(GROUP_LABEL_KEY[section.key])}
              </SidebarGroupLabel>
              {renderItems(section.items)}
            </div>
          ) : (
            <div key={`items-${index}`} className="flex flex-col gap-0.5">
              {renderItems(section.items)}
            </div>
          ),
        )}
      </nav>
    </aside>
  )
}
