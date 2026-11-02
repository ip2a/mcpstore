import {
  ActivityIcon,
  CableIcon,
  DatabaseIcon,
  FileStackIcon,
  HistoryIcon,
  HouseIcon,
  PuzzleIcon,
  ScrollTextIcon,
  ServerIcon,
  SettingsIcon,
  WaypointsIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react"
import type { ServiceAddress } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

export type AppView =
  | { name: "home" }
  | { name: "services" }
  | { name: "clients" }
  | { name: "plugins"; tab?: "installed" | "services" | "agents" }
  | { name: "tools" }
  | { name: "tool"; instanceId: string; toolName: string }
  | { name: "resources" }
  | { name: "resource"; instanceId: string; resourceUri: string }
  | { name: "prompts" }
  | { name: "prompt"; instanceId: string; promptName: string }
  | { name: "gateway" }
  | { name: "history" }
  | { name: "traces" }
  | { name: "cache" }
  | { name: "settings" }
  | { name: "instance"; addr: ServiceAddress }

export type NavItem = {
  view: Exclude<AppView, { name: "instance" }>
  label: string
  icon: LucideIcon
}

export function useNavItems(): NavItem[] {
  const { t } = useI18n()
  return [
    { view: { name: "home" }, label: t("navHome"), icon: HouseIcon },
    { view: { name: "services" }, label: t("navServices"), icon: ServerIcon },
    { view: { name: "clients" }, label: t("navClients"), icon: CableIcon },
    { view: { name: "gateway" }, label: t("gateway"), icon: WaypointsIcon },
    { view: { name: "tools" }, label: t("navTools"), icon: WrenchIcon },
    { view: { name: "resources" }, label: t("navResources"), icon: FileStackIcon },
    { view: { name: "prompts" }, label: t("navPrompts"), icon: ScrollTextIcon },
    { view: { name: "history" }, label: t("navHistory"), icon: HistoryIcon },
    { view: { name: "traces" }, label: t("navTraces"), icon: ActivityIcon },
    { view: { name: "cache" }, label: t("navCache"), icon: DatabaseIcon },
    { view: { name: "plugins" }, label: t("navPlugins"), icon: PuzzleIcon },
    { view: { name: "settings" }, label: t("settings"), icon: SettingsIcon },
  ]
}

export function useViewTitle(view: AppView): string {
  const { t } = useI18n()
  const items = useNavItems()
  if (view.name === "instance") return view.addr.service_name
  if (view.name === "tool") return view.toolName
  if (view.name === "resource") return view.resourceUri
  if (view.name === "prompt") return view.promptName
  return items.find((item) => item.view.name === view.name)?.label || t("navServices")
}
