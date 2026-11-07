import {
  ActivityIcon,
  CableIcon,
  FileStackIcon,
  HouseIcon,
  LayersIcon,
  PuzzleIcon,
  ScrollTextIcon,
  ServerIcon,
  SettingsIcon,
  WaypointsIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react"
import type { ServiceAddress } from "@/lib/api"
import type { I18nKey } from "@/lib/i18n-core"
import { useI18n } from "@/lib/i18n-context"

export type AppView =
  | { name: "home" }
  | { name: "services" }
  | { name: "scope" }
  | { name: "clients" }
  | { name: "plugins"; tab?: "installed" | "services" | "agents" }
  | { name: "tools" }
  | { name: "tool"; instanceId: string; toolName: string }
  | { name: "resources" }
  | { name: "resource"; instanceId: string; resourceUri: string }
  | { name: "prompts" }
  | { name: "prompt"; instanceId: string; promptName: string }
  | { name: "gateway" }
  | { name: "traces"; tab?: "table" | "graph" | "history" | "cache" }
  | { name: "settings" }
  | { name: "instance"; addr: ServiceAddress }

export type NavViewName = Exclude<AppView["name"], "instance" | "tool" | "resource" | "prompt">
export type NavGroupKey = "work" | "extensions" | "primitives" | "observability" | "config"

type NavEntry = { view: NavViewName; labelKey: I18nKey; icon: LucideIcon; group: NavGroupKey }

/** Single source of truth for the sidebar: display order, grouping, labels, icons. */
const NAV_ENTRIES: NavEntry[] = [
  { view: "home", labelKey: "navHome", icon: HouseIcon, group: "work" },
  { view: "services", labelKey: "navServices", icon: ServerIcon, group: "work" },
  { view: "scope", labelKey: "navAgents", icon: LayersIcon, group: "work" },
  { view: "clients", labelKey: "navClients", icon: CableIcon, group: "work" },
  { view: "gateway", labelKey: "gateway", icon: WaypointsIcon, group: "work" },
  { view: "plugins", labelKey: "navPlugins", icon: PuzzleIcon, group: "extensions" },
  { view: "tools", labelKey: "navTools", icon: WrenchIcon, group: "primitives" },
  { view: "resources", labelKey: "navResources", icon: FileStackIcon, group: "primitives" },
  { view: "prompts", labelKey: "navPrompts", icon: ScrollTextIcon, group: "primitives" },
  { view: "traces", labelKey: "navTraces", icon: ActivityIcon, group: "observability" },
  { view: "settings", labelKey: "settings", icon: SettingsIcon, group: "config" },
]

const NAV_GROUP_ORDER: { key: NavGroupKey; labelKey: I18nKey }[] = [
  { key: "work", labelKey: "navGroupWork" },
  { key: "primitives", labelKey: "navGroupPrimitives" },
  { key: "observability", labelKey: "navGroupObservability" },
  { key: "extensions", labelKey: "navGroupExtensions" },
  { key: "config", labelKey: "navGroupConfig" },
]

export type NavItem = {
  view: Exclude<AppView, { name: "instance" }>
  label: string
  icon: LucideIcon
}

export type NavSection = { labelKey: I18nKey; items: NavItem[] }

function navItem(t: (key: I18nKey) => string, { labelKey, icon, view }: NavEntry): NavItem {
  return { view: { name: view }, label: t(labelKey), icon }
}

export function useNavItems(): NavItem[] {
  const { t } = useI18n()
  return NAV_ENTRIES.map((entry) => navItem(t, entry))
}

export function useNavSections(): NavSection[] {
  const { t } = useI18n()
  return NAV_GROUP_ORDER.map(({ key, labelKey }) => ({
    labelKey,
    items: NAV_ENTRIES.filter((entry) => entry.group === key).map((entry) => navItem(t, entry)),
  }))
}

export function useViewTitle(view: AppView): string {
  const { t } = useI18n()
  if (view.name === "instance") return view.addr.service_name
  if (view.name === "tool") return view.toolName
  if (view.name === "resource") return view.resourceUri
  if (view.name === "prompt") return view.promptName
  const entry = NAV_ENTRIES.find((item) => item.view === view.name)
  return entry ? t(entry.labelKey) : t("navServices")
}
